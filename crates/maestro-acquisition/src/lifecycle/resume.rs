//! Resume composes current admission with kernel-owned captures, receipts and fences.
use crate::{
    CheckedPolicy, Refusal,
    policy::{
        authority::{Authority, Operation, Target},
        decision::{AdmissionControls, ItemAttributes, Request, RequestKind, admit},
        format_time,
    },
};
use maestro_kernel::{
    acquisition::{
        CaptureContext, CaptureEnvelope, Captures, Handle, Item, Receipts, RevisionLink,
    },
    artifact::Digest,
    retrieval::Clock,
    scope::Scope,
    store::Database,
};
use serde_json::Value;
use std::{
    process::{Child, ExitStatus},
    thread,
    time::{Duration, Instant, SystemTime},
};

/// Trusted current context; retained policy evidence never supplies authority.
#[derive(Debug)]
pub struct Current<'a> {
    /// Current checked policy, not the crashed attempt's frozen policy.
    pub policy: &'a CheckedPolicy,
    /// Current N07 controls; offline reuse need not request fresh robots bytes.
    pub controls: &'a dyn AdmissionControls,
    /// Separate current N05 authority.
    pub authority: &'a dyn Authority,
    /// Host-authenticated principal.
    pub principal: &'a str,
    /// Exact current collection scope.
    pub scope: &'a Scope,
    /// Explicit isolated account role.
    pub account: &'a str,
    /// Current host-bound request authorization context.
    pub authorization: &'a Digest,
    /// Injected trusted authority time, never source time.
    pub now: SystemTime,
    /// Current frozen host inputs, including caller, policy and profile digests.
    pub inputs: Handle,
}
impl Current<'_> {
    /// Reapply current authority, admission and representation to retained work.
    /// # Errors
    /// Revoked authority, tightened policy or changed request context refuses.
    pub fn check(&self, item: &Item) -> Result<(), Refusal> {
        let now = format_time(self.now)?;
        let request = Request {
            source_id: &item.source,
            url: &item.request.fetch_identity,
            kind: RequestKind::Resume,
            attributes: ItemAttributes::default(),
            cache_bypass: false,
            now: &now,
        };
        let admitted = admit(self.policy, &request, self.controls)?;
        let source = self
            .policy
            .policy()
            .sources
            .iter()
            .find(|source| source.id == item.source)
            .ok_or(Refusal::Access)?;
        if item.request.authorization_context != *self.authorization
            || item.request.representation_profile != source.acquisition_profile.digest
        {
            return Err(Refusal::Access);
        }
        let mut resource = admitted.identity().url().clone();
        resource.set_query(None);
        resource.set_fragment(None);
        let target = Target {
            scope: self.scope.to_string(),
            source: item.source.clone(),
            account: self.account.into(),
            resource: resource.to_string(),
        };
        target.validate()?;
        self.authority
            .decide(self.principal, Operation::Fetch, &target, self.now)
            .map_err(|_| Refusal::Access)?;
        Ok(())
    }
}

/// Reuse only verified acknowledged stages; missing evidence stays pending.
/// # Errors
/// Current authority/policy or corrupt scoped evidence refuses.
pub fn completed(
    captures: &dyn Captures,
    current: &Current<'_>,
    item: &Item,
) -> Result<Option<Handle>, Refusal> {
    current.check(item)?;
    captures
        .capture_for(current.scope, item)
        .map_err(|_| Refusal::Digest)
}

/// Resume prepared bytes through the current source/item epochs, without refetch.
/// This does not acknowledge completion or infer downstream success.
/// # Errors
/// Changed authority/policy/context, stale epochs or damaged bytes refuses.
pub fn prepared(
    captures: &(impl Captures + Receipts),
    current: &Current<'_>,
    owned: (&Item, &CaptureContext),
    capture: Handle,
    max_bytes: u64,
) -> Result<(CaptureEnvelope, Option<Vec<u8>>), Refusal> {
    let (item, context) = owned;
    current.check(item)?;
    if item.source != context.writer.source {
        return Err(Refusal::Invalid);
    }
    if captures
        .prepared_for(current.scope, item)
        .map_err(|_| Refusal::Digest)?
        != Some(capture)
    {
        return Err(Refusal::Access);
    }
    let result = captures
        .read_capture(context, capture, max_bytes)
        .map_err(|_| Refusal::Digest)?;
    if !same_inputs(captures, current.principal, result.0.inputs, current.inputs)? {
        return Err(Refusal::Access);
    }
    Ok(result)
}

/// Compare N36's frozen authority/policy/profile closure, not run time or mode.
/// Identical opaque input bytes remain valid for other trusted hosts' schemas.
/// # Errors
/// Denied, missing, corrupt or unparsable differing inputs cannot prove equality.
pub fn same_inputs(
    store: &dyn Receipts,
    principal: &str,
    previous: Handle,
    current: Handle,
) -> Result<bool, Refusal> {
    let previous = store
        .read(principal, previous)
        .map_err(|_| Refusal::Digest)?
        .ok_or(Refusal::Access)?;
    let current = store
        .read(principal, current)
        .map_err(|_| Refusal::Digest)?
        .ok_or(Refusal::Access)?;
    if previous.bytes() == current.bytes() {
        return Ok(true);
    }
    let previous: Value = serde_json::from_slice(previous.bytes()).map_err(|_| Refusal::Invalid)?;
    let current: Value = serde_json::from_slice(current.bytes()).map_err(|_| Refusal::Invalid)?;
    Ok([
        "collection",
        "resources",
        "os_principal",
        "kernel_principal",
        "scope",
    ]
    .iter()
    .all(|key| previous.get(key).is_some() && previous.get(key) == current.get(key)))
}

/// Explicit historical selection, never a capture-stage completion disposition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedPrior {
    /// Exact S1 revision requested by the caller.
    pub revision: String,
    /// Verified N26 lineage, not a replacement or fallback revision.
    pub links: Vec<RevisionLink>,
}
impl SelectedPrior {
    /// Distinct owner report; it cannot claim fresh verification.
    #[must_use]
    pub fn report(&self) -> String {
        format!(
            "selected prior revision {}, not freshly verified",
            self.revision
        )
    }
}
/// Resolve only an explicit S1 revision under current grants and artifact integrity.
/// # Errors
/// Missing/failed revision, denied scope, stale generation or corrupt evidence refuses.
pub fn select_prior(
    db: &Database,
    principal: &str,
    revision: &str,
) -> Result<SelectedPrior, Refusal> {
    let scopes = db.visible(principal).map_err(|_| Refusal::Access)?;
    let links = db
        .verified_revision_links(&scopes, revision)
        .map_err(|_| Refusal::Digest)?;
    if links.is_empty() {
        return Err(Refusal::Missing);
    }
    Ok(SelectedPrior {
        revision: revision.into(),
        links,
    })
}

/// Stop and reap only the child handle this invocation owns, within one deadline.
/// A caller must retain ownership on error; timeout is not cancellation success.
/// # Errors
/// Kill/wait failure or an exhausted trusted deadline refuses acknowledgement.
pub fn stop_owned(
    child: &mut Child,
    deadline: Instant,
    clock: &dyn Clock,
) -> Result<ExitStatus, Refusal> {
    if let Some(status) = child.try_wait().map_err(|_| Refusal::Unsupported)? {
        return Ok(status);
    }
    if clock.now() >= deadline {
        return Err(Refusal::Deadline);
    }
    child.kill().map_err(|_| Refusal::Unsupported)?;
    loop {
        if let Some(status) = child.try_wait().map_err(|_| Refusal::Unsupported)? {
            return Ok(status);
        }
        if clock.now() >= deadline {
            return Err(Refusal::Deadline);
        }
        thread::sleep(Duration::from_millis(1));
    }
}
