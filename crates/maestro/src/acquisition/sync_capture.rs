//! Public HTTP, immutable capture and offline discovery under one current writer.
use super::{
    controls::{SourceWork, decision, millis, request, storage},
    output::{Entry, Report},
};
use crate::failure::Failure;
use maestro_acquisition::{
    capture::{CaptureBudget, http_envelope, prepare},
    discovery::{links::DomLinks, partition::discover},
    lifecycle::resources::Reservation,
    policy::{format_time, identity::FetchIdentity, limits::Limits},
    transport::{
        budget::Usage,
        connect::PinnedTransport,
        http::{Fetch, Http, Response},
        pacing::PacingContext,
        stream::Accounting,
    },
};
use maestro_kernel::{
    acquisition::{
        CaptureContext, CaptureEnvelope, Captures, DispatchRequest, Enumeration, Frontier, Handle,
        Item, ItemDisposition, LeaseRequest, Partition, Partitions, Receipts, Representation,
        SafeIdentity, SourceLease, StageItem, Transport, Window,
    },
    artifact::Digest,
};
use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime},
};
use tokio::io::{AsyncRead, AsyncWrite};

/// One admitted source allocation and its durable discovery checkpoints.
pub(crate) struct CaptureWork<'a, 'b, S, T> {
    /// Existing source composition and ports.
    pub(crate) source: &'a SourceWork<'b, S, T>,
    /// Current fenced writer.
    pub(crate) writer: &'a SourceLease,
    /// N11 owned resource reservation.
    pub(crate) reservation: &'a mut Reservation,
    /// Tightened OA3/source limits.
    pub(crate) bounds: &'a [Limits],
    /// Cumulative HTTP attempts, elapsed time and response bytes.
    pub(crate) accounting: &'a mut Accounting,
    /// Actual retained allocation, never reset between captures.
    pub(crate) usage: &'a mut Usage,
    /// Current run checkpoints and parent depths.
    pub(crate) partitions: &'a mut Vec<(Handle, u64)>,
    /// Remaining bounded inventory slots before starting discovery.
    pub(crate) remaining: u64,
}
/// Verified reuse is not revalidation, and starts no fetch.
fn reuse<S: Captures + Receipts, T>(
    work: &CaptureWork<'_, '_, S, T>,
    item: &Item,
    report: &mut Report,
) -> Result<Option<StageItem>, Failure> {
    if item.capture.is_none() {
        return Ok(None);
    }
    let evidence = work
        .source
        .store
        .capture_for(work.source.scope, item)
        .map_err(|_| storage())?;
    let Some(handle) = evidence else {
        return Ok(None);
    };
    let bytes = work
        .source
        .store
        .read(work.source.runtime.kernel_principal, handle)
        .map_err(|_| storage())?
        .ok_or_else(storage)?;
    let envelope: CaptureEnvelope = serde_json::from_slice(bytes.bytes()).map_err(|_| storage())?;
    let mut entry = Entry::new(
        &item.request.fetch_identity,
        "captured_earlier_not_revalidated",
    );
    entry.observed_ms = Some(envelope.observed_ms);
    report.completed.push(entry);
    Ok(Some(StageItem {
        item: item.id.to_string().parse().map_err(|_| storage())?,
        disposition: ItemDisposition::Unchanged,
        evidence: Some(handle),
    }))
}
/// Every transport failure leaves the original frontier item pending.
pub(crate) async fn capture<S, T>(
    work: &mut CaptureWork<'_, '_, S, T>,
    item: &Item,
    depth: Option<u64>,
    report: &mut Report,
) -> Result<StageItem, Failure>
where
    S: Frontier + Captures + Partitions + Receipts,
    T: PinnedTransport,
    T::Connection: AsyncRead + AsyncWrite + Unpin + Send,
{
    if let Some(reused) = reuse(work, item, report)? {
        return Ok(reused);
    }
    let mut stage = StageItem {
        item: item.id.to_string().parse().map_err(|_| storage())?,
        disposition: ItemDisposition::Pending,
        evidence: None,
    };
    if item.capture.is_some() {
        report.pending.push(Entry::new(
            &item.request.fetch_identity,
            "capture_unverifiable",
        ));
        return Ok(stage);
    }
    let Some((context, response)) = fetch_item(work, item, report).await? else {
        return Ok(stage);
    };
    let mut envelope = envelope(work, item)?;
    http_envelope(&mut envelope, &response).map_err(|_| storage())?;
    let mut budget = CaptureBudget {
        reservation: work.reservation,
        bounds: work.bounds,
        usage: *work.usage,
    };
    let prepared = prepare(
        work.source.store,
        work.source.policy,
        &context,
        (&envelope, &response.body),
        &mut budget,
    );
    *work.usage = budget.usage;
    let Ok(handle) = prepared else {
        report.pending.push(Entry::new(
            &item.request.fetch_identity,
            "capture_budget_or_verification",
        ));
        return Ok(stage);
    };
    stage.evidence = Some(handle);
    let acknowledged = if let Some(depth) = depth {
        discovery(work, (&context, handle), &envelope, depth, report).await?
    } else if work.source.source.discovery.is_empty()
        || envelope
            .declared_media
            .as_deref()
            .is_some_and(|media| media != "text/html")
    {
        discovery(work, (&context, handle), &envelope, 0, report).await?
    } else {
        report.pending.push(Entry::new(
            &item.request.fetch_identity,
            "discovery_depth_unknown",
        ));
        false
    };
    if acknowledged {
        stage.disposition = ItemDisposition::Accepted;
    }
    let mut entry = Entry::new(
        &item.request.fetch_identity,
        if acknowledged {
            "captured"
        } else {
            "captured_discovery_pending"
        },
    );
    entry.observed_ms = Some(envelope.observed_ms);
    report.completed.push(entry);
    Ok(stage)
}
/// Fresh readiness, leases and the actual N09 operation share the same guards.
async fn fetch_item<S, T>(
    work: &mut CaptureWork<'_, '_, S, T>,
    item: &Item,
    report: &mut Report,
) -> Result<Option<(CaptureContext, Response)>, Failure>
where
    S: Frontier + Captures + Partitions + Receipts,
    T: PinnedTransport,
    T::Connection: AsyncRead + AsyncWrite + Unpin + Send,
{
    let now = SystemTime::now();
    let text = format_time(now).map_err(|_| storage())?;
    let candidate = request(&work.source.source.id, &item.request.fetch_identity, &text);
    let identity =
        FetchIdentity::parse(work.source.source, candidate.url).map_err(|_| storage())?;
    let readiness = decision(work.source.policy, &candidate, work.source.runtime.controls);
    if readiness == "robots_unavailable" {
        ensure_robots(work, &identity, &text, now).await?;
    }
    let readiness = decision(work.source.policy, &candidate, work.source.runtime.controls);
    if readiness != "allowed" {
        report.pending.push(Entry::new(candidate.url, readiness));
        return Ok(None);
    }
    if work
        .reservation
        .checkpoint(work.bounds, *work.usage)
        .is_err()
    {
        report.pending.push(Entry::new(candidate.url, "budget"));
        return Ok(None);
    }
    let holder = work.source.receipt.attempt.to_string();
    let dispatch = work.source.store.lease(
        work.writer,
        item.id,
        DispatchRequest {
            lease: LeaseRequest {
                holder: &holder,
                now,
                term: Duration::from_millis(work.accounting.limits().elapsed_ms.get())
                    .min(Duration::from_hours(1)),
            },
            max_attempts: u32::try_from(work.accounting.limits().requests.get())
                .unwrap_or(u32::MAX),
        },
    );
    let Ok(dispatch) = dispatch else {
        report
            .pending
            .push(Entry::new(candidate.url, "lease_or_attempt_budget"));
        return Ok(None);
    };
    let fetch = Fetch {
        request: candidate,
        principal: work.source.principal.id,
        scope: work.source.scope.as_str(),
        account: "public",
        authority_time: now,
        credentials: None,
        robots: false,
    };
    let response = match http(work).fetch(&fetch, work.accounting).await {
        Ok(response) if (200..300).contains(&response.status) => response,
        Ok(_) | Err(_) => {
            report
                .pending
                .push(Entry::new(&item.request.fetch_identity, "transport"));
            return Ok(None);
        }
    };
    Ok(Some((
        CaptureContext {
            writer: work.writer.clone(),
            item: dispatch,
            now: SystemTime::now(),
        },
        response,
    )))
}
/// N10 rules are obtained only through N09's admitted, paced operation.
async fn ensure_robots<S, T>(
    work: &mut CaptureWork<'_, '_, S, T>,
    identity: &FetchIdentity,
    text: &str,
    now: SystemTime,
) -> Result<(), Failure>
where
    T: PinnedTransport,
    T::Connection: AsyncRead + AsyncWrite + Unpin + Send,
{
    let http = http(work);
    let mut url = identity.url().clone();
    url.set_path("/robots.txt");
    url.set_query(None);
    url.set_fragment(None);
    let robots = Fetch {
        request: request(&work.source.source.id, url.as_str(), text),
        principal: work.source.principal.id,
        scope: work.source.scope.as_str(),
        account: "public",
        authority_time: now,
        credentials: None,
        robots: true,
    };
    if let Ok(cache) = http
        .fetch_robots(&robots, work.accounting, millis().map_err(|_| storage())?)
        .await
    {
        work.source.runtime.controls.insert(cache);
    }
    Ok(())
}
/// Build the immutable N12 envelope; N09 supplies all response metadata.
fn envelope<S, T>(
    work: &CaptureWork<'_, '_, S, T>,
    item: &Item,
) -> Result<CaptureEnvelope, Failure> {
    Ok(CaptureEnvelope {
        schema: "maestro-capture/1".into(),
        source: work.source.source.id.clone(),
        item: item.id.to_string().parse().map_err(|_| storage())?,
        run: work.source.receipt.run,
        requested: SafeIdentity::new(&item.request.fetch_identity).map_err(|_| storage())?,
        final_identity: SafeIdentity::new(&item.request.fetch_identity).map_err(|_| storage())?,
        redirects: vec![],
        status: 200,
        headers: BTreeMap::new(),
        declared_media: None,
        detected_media: None,
        artifact: Digest::of(b""),
        length: 0,
        observed_ms: millis().map_err(|_| storage())?,
        transport: Transport::Http,
        profile: item.request.representation_profile.clone(),
        authorization_context: item.request.authorization_context.clone(),
        representation: Representation::WireBody,
        parent: None,
        inputs: work.source.receipt.inputs,
        access: work.source.receipt.inputs,
        decision: work.source.receipt.inputs,
    })
}
/// N09 owns every actual hop; the readiness projection is never a fetch control.
fn http<'a, S, T>(work: &CaptureWork<'_, 'a, S, T>) -> Http<'a, T> {
    let run_id = work.source.run_id;
    Http {
        policy: work.source.policy,
        controls: work.source.runtime.controls,
        authority: work.source.runtime.authority,
        resolver: work.source.runtime.resolver,
        transport: work.source.runtime.transport,
        pacing: work.source.runtime.pacing,
        pacing_context: PacingContext {
            run_id,
            epoch: work.source.runtime.epoch,
            started_ms: 0,
            deadline_ms: work
                .source
                .policy
                .policy()
                .aggregate_limits
                .elapsed_ms
                .get(),
        },
    }
}
/// The sole N13 enumerator acknowledges only stable bounded discovery.
async fn discovery<S, T>(
    work: &mut CaptureWork<'_, '_, S, T>,
    capture: (&CaptureContext, Handle),
    envelope: &CaptureEnvelope,
    depth: u64,
    report: &mut Report,
) -> Result<bool, Failure>
where
    S: Frontier + Captures + Partitions + Receipts,
{
    if !work.source.source.discovery.is_empty() && envelope.declared_media.is_none() {
        report.pending.push(Entry::new(
            envelope.requested.as_str(),
            "discovery_media_unknown",
        ));
        return Ok(false);
    }
    if envelope.declared_media.as_deref() != Some("text/html")
        || work.source.source.discovery.is_empty()
    {
        work.source
            .store
            .acknowledge_capture(capture.0, capture.1)
            .map_err(|_| storage())?;
        return Ok(true);
    }
    if work.partitions.len() as u64 >= work.accounting.limits().partitions.get() {
        report.pending.push(Entry::new(
            envelope.requested.as_str(),
            "inventory_or_partition_limit",
        ));
        return Ok(false);
    }
    let partition = Partition {
        id: Handle::new(),
        run: work.source.receipt.run,
        kind: Enumeration::Links,
        window: Window {
            start: 0,
            end: 1,
            overlap: 0,
            skew: 0,
        },
        max_batches: 1,
        max_items: u16::try_from(work.remaining.min(999)).map_err(|_| storage())?,
    };
    let extractor = DomLinks::new().map_err(|_| storage())?;
    let discovered = discover(
        work.source.store,
        &extractor,
        capture,
        work.source.policy,
        (partition, depth),
    )
    .await;
    let Ok(batch) = discovered else {
        report.pending.push(Entry::new(
            envelope.requested.as_str(),
            "discovery_unavailable",
        ));
        return Ok(false);
    };
    work.partitions.push((batch.partition.id, depth));
    for excluded in &batch.not_enqueued {
        report.not_enqueued(excluded).map_err(|_| storage())?;
    }
    if batch.inventory_overflow > 0 {
        report.overflow = report
            .overflow
            .checked_add(u64::from(batch.inventory_overflow))
            .ok_or_else(storage)?;
    }
    if !batch.stable || batch.truncated {
        report.pending.push(Entry::new(
            envelope.requested.as_str(),
            "discovery_incomplete",
        ));
        return Ok(false);
    }
    Ok(true)
}
