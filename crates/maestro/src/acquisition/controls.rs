//! Shared admission and borrowed acquisition port context; no authoritative queue.
use crate::failure::Failure;
use maestro_acquisition::{
    CheckedPolicy, Principal, Refusal,
    lifecycle::resources::{Reservation, Resources},
    policy::{
        authority::{Authority, Operation, Target},
        decision::{AdmissionControls, ItemAttributes, Request, RequestKind, admit},
        identity::FetchIdentity,
        limits::Limits,
        source::Source,
    },
    transport::{
        budget::Usage,
        connect::Resolver,
        pacing::OriginLedger,
        robots::{DenyOverrides, RobotsBinding, RobotsCache},
        robots_store::RobotsStore,
        stream::Accounting,
    },
};
use maestro_kernel::{
    acquisition::{Batch, Handle, Receipt, SourceLease},
    artifact::Digest,
    scope::Scope,
};
use std::{
    cell::RefCell,
    num::NonZeroUsize,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

/// One public operation context; no credential facility is bound.
#[derive(Debug)]
pub(crate) struct Controls<'a> {
    /// Checked immutable policy, shared with HTTP.
    policy: &'a CheckedPolicy,
    /// Current platform-authenticated authority.
    authority: &'a dyn Authority,
    /// Caller identity supplied outside source documents.
    principal: &'a str,
    /// Current collection read scope.
    scope: &'a str,
    /// Bounded N10 evidence, never a second transport.
    robots: RefCell<RobotsStore>,
}
impl<'a> Controls<'a> {
    /// Bind existing ports without resolving DNS, fetching or obtaining credentials.
    pub(crate) fn new(
        policy: &'a CheckedPolicy,
        authority: &'a dyn Authority,
        principal: &'a str,
        scope: &'a str,
    ) -> Self {
        Self {
            policy,
            authority,
            principal,
            scope,
            robots: RefCell::new(RobotsStore::new(NonZeroUsize::MIN.saturating_add(999))),
        }
    }
    /// Retain only N09's admitted robots outcome.
    pub(crate) fn insert(&self, cache: RobotsCache) {
        self.robots.borrow_mut().insert(cache);
    }
    /// A cache miss requires an admitted robots fetch, never a guessed allow.
    pub(crate) fn has_robots(&self, identity: &FetchIdentity) -> Result<bool, Refusal> {
        let binding = RobotsBinding::new(self.policy, identity.source_id())?;
        Ok(self.robots.borrow().get(identity, binding).is_some())
    }
    /// Distinguish missing rules from actual robots refusal in owner output.
    pub(crate) fn robots_reason(&self, identity: &FetchIdentity) -> &'static str {
        if self.has_robots(identity).unwrap_or(false) {
            "robots_denied"
        } else {
            "robots_unavailable"
        }
    }
}
impl AdmissionControls for Controls<'_> {
    fn caller(&self, source: &Source, request: &Request<'_>) -> Result<(), Refusal> {
        if source.auth_role.is_some() {
            return Err(Refusal::Unsupported);
        }
        // N05 exact targets omit query/fragment, while N07 keeps fetch semantics.
        let resource = request
            .url
            .split(['?', '#'])
            .next()
            .ok_or(Refusal::Invalid)?;
        let target = Target {
            scope: self.scope.into(),
            source: source.id.clone(),
            account: "public".into(),
            resource: resource.into(),
        };
        self.authority
            .decide(self.principal, Operation::Fetch, &target, SystemTime::now())
            .map_err(|_| Refusal::Access)?;
        Ok(())
    }
    fn network(&self, _: &FetchIdentity) -> Result<(), Refusal> {
        // Offline admission has no DNS. N09/N08 classify and pin every real hop.
        Ok(())
    }
    fn robots(&self, identity: &FetchIdentity) -> Result<(), Refusal> {
        let binding = RobotsBinding::new(self.policy, identity.source_id())?;
        self.robots
            .borrow()
            .get(identity, binding)
            .ok_or(Refusal::Missing)?
            .check(identity, binding, millis()?, &DenyOverrides)
    }
}
/// Trusted wall time, shared by observations and TTL checks.
pub(crate) fn millis() -> Result<u64, Refusal> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Refusal::Invalid)?
            .as_millis(),
    )
    .map_err(|_| Refusal::Invalid)
}

/// Replaceable HTTP ports; no connector host, credential provider or embedding port.
pub(crate) struct Runtime<'a, T> {
    /// N05 read-only exact authority.
    pub(crate) authority: &'a dyn Authority,
    /// N08 fresh checked resolution.
    pub(crate) resolver: &'a dyn Resolver,
    /// Pinned public connection adapter.
    pub(crate) transport: &'a T,
    /// One shared origin budget across sources and robots/content.
    pub(crate) pacing: &'a OriginLedger,
    /// Same controls for preview and actual HTTP admission.
    pub(crate) controls: &'a Controls<'a>,
    /// Frontier page size: production freezes 1,000; tests exercise smaller cursors.
    pub(crate) frontier_page_size: u16,
    /// Frozen kernel mapping of this OS caller; never a manifest-selected reader.
    pub(crate) kernel_principal: &'a str,
    /// Exact portable collection declaration bytes frozen for this operation.
    pub(crate) collection: Digest,
    /// One trusted run time domain.
    pub(crate) epoch: Instant,
}
/// The existing pure admission request, used by preview and current dispatch.
pub(crate) fn request<'a>(source: &'a str, url: &'a str, now: &'a str) -> Request<'a> {
    Request {
        source_id: source,
        url,
        kind: RequestKind::Resume,
        attributes: ItemAttributes::default(),
        cache_bypass: false,
        now,
    }
}
/// Separate current policy eligibility from robots readiness, never used for fetching.
#[derive(Debug)]
struct Readiness<'a>(&'a Controls<'a>);
impl AdmissionControls for Readiness<'_> {
    fn caller(&self, source: &Source, request: &Request<'_>) -> Result<(), Refusal> {
        self.0.caller(source, request)
    }
    fn network(&self, identity: &FetchIdentity) -> Result<(), Refusal> {
        self.0.network(identity)
    }
    fn robots(&self, _: &FetchIdentity) -> Result<(), Refusal> {
        Ok(())
    }
}
/// Explain decisions without interpreting a robots denial as a policy exclusion.
pub(crate) fn decision(
    policy: &CheckedPolicy,
    candidate: &Request<'_>,
    controls: &Controls<'_>,
) -> &'static str {
    let Some(source) = policy
        .policy()
        .sources
        .iter()
        .find(|source| source.id == candidate.source_id)
    else {
        return "unsupported";
    };
    if controls.caller(source, candidate).is_err() {
        return "authority";
    }
    match admit(policy, candidate, &Readiness(controls)) {
        Ok(allowed) => {
            if controls.robots(allowed.identity()).is_err() {
                controls.robots_reason(allowed.identity())
            } else {
                "allowed"
            }
        }
        Err(Refusal::Access) => "policy_denial",
        Err(_) => "unresolved_identity",
    }
}
/// Fixed diagnostics never echo source bytes, paths or transport errors.
pub(crate) fn storage() -> Failure {
    Failure::failed("acquisition storage failed")
}

/// Borrowed composition context, kept out of portable policy documents.
pub(crate) struct SourceWork<'a, S, T> {
    /// Existing kernel ports.
    pub(crate) store: &'a S,
    /// Validated baseline.
    pub(crate) policy: &'a CheckedPolicy,
    /// Current reader context.
    pub(crate) principal: &'a Principal<'a>,
    /// Shared admitted HTTP ports.
    pub(crate) runtime: &'a Runtime<'a, T>,
    /// Shared N11 resource controls.
    pub(crate) resources: &'a Resources,
    /// Exact authorized collection.
    pub(crate) scope: &'a Scope,
    /// Checked source definition.
    pub(crate) source: &'a Source,
    /// Cleanup owner releases only after persisted dispositions or a stopped error.
    pub(crate) writers: &'a mut Vec<SourceLease>,
    /// Unique logical pacing identity shared by every source in this run.
    pub(crate) run_id: &'a str,
    /// Unique frozen pending receipt.
    pub(crate) receipt: &'a Receipt,
}

/// One admitted source allocation and its durable discovery checkpoints.
pub(crate) struct CaptureWork<'a, 'b, S, T> {
    /// Prior immutable checkpoints for verified offline continuation.
    pub(crate) checkpoints: &'a [Batch],
    /// Existing source composition and ports.
    pub(crate) source: &'a SourceWork<'b, S, T>,
    /// Current fenced writer.
    pub(crate) writer: &'a SourceLease,
    /// N11 owned resource reservation.
    pub(crate) reservation: &'a mut Reservation,
    /// Tightened OA3/source limits.
    pub(crate) bounds: &'a [Limits],
    /// Prior sources' retained staging allocation.
    pub(crate) carried_staging: u64,
    /// Complete aggregate reservation bounds, never narrowed by a finished source.
    pub(crate) aggregate_bounds: &'a [Limits],
    /// Cumulative HTTP attempts, elapsed time and response bytes.
    pub(crate) accounting: &'a mut Accounting,
    /// Actual retained allocation, never reset between captures.
    pub(crate) usage: &'a mut Usage,
    /// Current run checkpoints and parent depths.
    pub(crate) partitions: &'a mut Vec<(Handle, u64)>,
}
