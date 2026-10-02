//! Captured-page discovery acknowledges only after a durable bounded checkpoint.
use super::links::LinkExtractor;
use crate::{
    CheckedPolicy, Refusal,
    policy::{
        decision::{ItemAttributes, check_eligibility},
        decisions::Decisions,
        identity::FetchIdentity,
        limits::Limits,
        source::{Discovery, Source},
        utc,
    },
    transport::budget::compose,
};
use maestro_kernel::{
    acquisition::{
        Batch, CaptureContext, CaptureEnvelope, Captures, ChangeKeys, DiscoveredItem, Enumeration,
        Handle, NewItem, NotEnqueued, NotEnqueuedReason, Partition, Partitions, ReceiptError,
    },
    artifact::Digest,
};
use std::collections::BTreeSet;
use url::Url;

/// Enumerate a current leased capture, checkpoint every eligible item, then acknowledge.
/// No transport starts here: N09 fetched the bytes, N12 captured them, and N09
/// must re-admit every later frontier dispatch under current authority/policy.
/// Completeness is relative to the recorded extractor selector contract only.
/// # Errors
/// Lost ownership, unsupported media, invalid policy or failed durable enqueue refuses.
pub async fn discover(
    store: &(impl Partitions + Captures),
    extractor: &dyn LinkExtractor,
    capture: (&CaptureContext, Handle),
    policy: &CheckedPolicy,
    request: (Partition, u64),
) -> Result<Batch, ReceiptError> {
    discover_with_captured(
        store,
        extractor,
        capture,
        policy,
        (request.0, request.1, &BTreeSet::new()),
    )
    .await
}
/// Enumerate the full bounded inventory, prioritizing children without captures.
/// # Errors
/// The same ownership, immutable capture and finite inventory guards as discovery.
pub async fn discover_with_captured(
    store: &(impl Partitions + Captures),
    extractor: &dyn LinkExtractor,
    capture: (&CaptureContext, Handle),
    policy: &CheckedPolicy,
    request: (Partition, u64, &BTreeSet<String>),
) -> Result<Batch, ReceiptError> {
    let (mut partition, depth, captured) = request;
    let (context, handle) = capture;
    let source = policy
        .policy()
        .sources
        .iter()
        .find(|source| source.id == context.writer.source)
        .ok_or(ReceiptError::Invalid)?;
    let limits = compose(&[
        source.limits.clone(),
        policy.policy().aggregate_limits.clone(),
    ])
    .map_err(|_| ReceiptError::Invalid)?;
    partition.max_items = partition.max_items.min(
        u16::try_from(source.limits.pages.get().min(1000)).map_err(|_| ReceiptError::Invalid)?,
    );
    partition.max_batches = partition
        .max_batches
        .min(u16::try_from(limits.partitions.get().min(1000)).map_err(|_| ReceiptError::Invalid)?);
    let max_bytes = capture_bound(&limits);
    let (envelope, bytes) = store.read_capture(context, handle, max_bytes)?;
    if envelope.profile != source.acquisition_profile.digest
        || envelope.declared_media.as_deref() != Some("text/html")
        || !(200..300).contains(&envelope.status)
    {
        return Err(ReceiptError::Invalid);
    }
    partition.run = envelope.run;
    partition.kind = Enumeration::Links;
    let mut batch = Batch {
        partition,
        cursor: None,
        next: None,
        terminal: true,
        stable: true,
        truncated: bytes.is_none(),
        expected: None,
        items: vec![],
        parent_depth: Some(depth),
        capture: Some(handle),
        extractor: Some(extractor.contract().to_owned()),
        parent_keys: Some(keys(&envelope, &[])?),
        not_enqueued: vec![],
        inventory_overflow: 0,
    };
    if let Some(bytes) = bytes {
        let output = extractor
            .extract(envelope.final_identity.as_str(), &bytes)
            .await?;
        if output.contract != extractor.contract() {
            return Err(ReceiptError::Invalid);
        }
        let mut links = output.links;
        links.sort_unstable();
        links.dedup();
        batch.parent_keys = Some(keys(&envelope, &links)?);
        let keys = child_keys(&envelope);
        let now = utc::format(context.now).map_err(|_| ReceiptError::Invalid)?;
        let stopped = depth_reason(source, limits.depth.get(), depth)?;
        let eligible = select_links(
            source,
            &policy.decisions,
            &links,
            (&now, stopped),
            &mut batch,
        );
        batch.expected = u16::try_from(eligible.len()).ok();
        batch.inventory_overflow = u32::try_from(
            eligible
                .len()
                .saturating_sub(usize::from(batch.partition.max_items)),
        )
        .map_err(|_| ReceiptError::Invalid)?;
        batch.truncated =
            output.limit_hit || batch.inventory_overflow > 0 || batch.not_enqueued.len() > 1000;
        batch.not_enqueued.truncate(1000);
        let mut eligible: Vec<_> = eligible.into_iter().collect();
        eligible.sort_by_key(|identity| captured.contains(identity));
        batch.items = eligible
            .into_iter()
            .take(usize::from(batch.partition.max_items))
            .map(|fetch_identity| DiscoveredItem {
                request: NewItem {
                    fetch_identity,
                    authorization_context: envelope.authorization_context.clone(),
                    representation_profile: envelope.profile.clone(),
                },
                keys: keys.clone(),
            })
            .collect();
    }
    store.checkpoint(&context.writer, &batch, context.now)?;
    if batch.stable && !batch.truncated {
        store.acknowledge_capture(context, handle)?;
    }
    Ok(batch)
}
/// Inventory unknown candidates as pending; only proven exclusions leave coverage stable.
fn select_links(
    source: &Source,
    registries: &[Decisions],
    links: &[String],
    context: (&str, Option<NotEnqueuedReason>),
    batch: &mut Batch,
) -> BTreeSet<String> {
    let (now, stopped) = context;
    let mut eligible = BTreeSet::new();
    for url in links {
        let candidate = classify_link(source, registries, url, now);
        let definitive = candidate.as_ref().err().copied().filter(|reason| {
            matches!(
                reason,
                NotEnqueuedReason::PolicyDenial | NotEnqueuedReason::NonFetchScheme
            )
        });
        let reason = definitive.or_else(|| {
            if stopped == Some(NotEnqueuedReason::BeyondDeclaredDepth) {
                stopped
            } else {
                candidate.as_ref().err().copied().or(stopped)
            }
        });
        if let Some(reason) = reason {
            record_reference(url, reason, batch);
            if reason.pending() {
                batch.stable = false;
            }
        } else if let Ok((identity, known)) = candidate {
            if !known {
                batch.stable = false;
            }
            eligible.insert(identity.as_str().to_owned());
        }
    }
    eligible
}
/// Only definite policy/scheme exclusions outrank the declared depth boundary.
fn classify_link(
    source: &Source,
    registries: &[Decisions],
    url: &str,
    now: &str,
) -> Result<(FetchIdentity, bool), NotEnqueuedReason> {
    if non_fetch_scheme(url) {
        return Err(NotEnqueuedReason::NonFetchScheme);
    }
    let identity = FetchIdentity::parse(source, url).map_err(|error| {
        if error == Refusal::Access {
            NotEnqueuedReason::PolicyDenial
        } else {
            NotEnqueuedReason::UnresolvedIdentity
        }
    })?;
    let admitted = check_eligibility(
        source,
        &identity,
        registries,
        ItemAttributes::default(),
        now,
    );
    if admitted == Err(Refusal::Access) {
        return Err(NotEnqueuedReason::PolicyDenial);
    }
    // A known identity with missing selector attributes remains durable pending inventory.
    Ok((identity, admitted.is_ok()))
}

/// Well-formed references outside acquisition's HTTPS fetch scheme are exclusions.
fn non_fetch_scheme(reference: &str) -> bool {
    Url::parse(reference).is_ok_and(|url| url.scheme() != "https")
}

/// Independent representation, metadata, permission, validator and canonical link keys.
fn keys(envelope: &CaptureEnvelope, links: &[String]) -> Result<ChangeKeys, ReceiptError> {
    Ok(ChangeKeys {
        revision: None,
        validator: Some(Digest::of(&serde_json::to_vec(&envelope.headers)?)),
        metadata: Some(Digest::of(&serde_json::to_vec(&(
            &envelope.declared_media,
            &envelope.detected_media,
        ))?)),
        permissions: envelope.authorization_context.clone(),
        links: Digest::of(&serde_json::to_vec(links)?),
        representation: Some(envelope.artifact.clone()),
    })
}

/// One content-free spelling shared by sync and inspect.
fn record_reference(reference: &str, reason: NotEnqueuedReason, batch: &mut Batch) {
    batch.not_enqueued.push(NotEnqueued {
        reference: Digest::of(reference.as_bytes()),
        reason,
    });
}

/// Declared scope is an exclusion; an earlier operational ceiling is a hold.
fn depth_reason(
    source: &Source,
    run_depth: u64,
    depth: u64,
) -> Result<Option<NotEnqueuedReason>, ReceiptError> {
    let declared = source
        .discovery
        .iter()
        .filter_map(|selection| {
            if let Discovery::Links { depth } = selection {
                Some(depth.get())
            } else {
                None
            }
        })
        .max()
        .ok_or(ReceiptError::Invalid)?;
    if depth >= declared {
        return Ok(Some(NotEnqueuedReason::BeyondDeclaredDepth));
    }
    if depth >= run_depth {
        return Ok(Some(NotEnqueuedReason::RunDepthLimit));
    }
    Ok(None)
}

/// A parent makes no representation or validator claims about its children.
fn child_keys(envelope: &CaptureEnvelope) -> ChangeKeys {
    ChangeKeys {
        revision: None,
        validator: None,
        metadata: None,
        permissions: envelope.authorization_context.clone(),
        links: Digest::of(b""),
        representation: None,
    }
}

/// Bound parser input by every applicable memory and byte ceiling.
fn capture_bound(limits: &Limits) -> u64 {
    limits
        .decode
        .expanded_bytes
        .get()
        .min(limits.decode.memory_bytes.get())
        .min(limits.dom_bytes.get())
        .min(limits.memory_bytes.get())
}
