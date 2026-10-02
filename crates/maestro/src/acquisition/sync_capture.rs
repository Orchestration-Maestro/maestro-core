//! Public HTTP, immutable capture and offline discovery under one current writer.
use super::{
    controls::{CaptureWork, Readiness, decision, millis, request, storage},
    output::{Entry, Report},
    sync_discovery::discovery,
};
use crate::failure::Failure;
use maestro_acquisition::{
    Refusal,
    capture::{CaptureBudget, http_envelope, prepare},
    lifecycle::{
        full::same_capture,
        resume::{Current, prepared},
    },
    policy::{format_time, identity::FetchIdentity},
    transport::{
        budget::Usage,
        connect::PinnedTransport,
        http::{Fetch, Http, Response},
        pacing::PacingContext,
    },
};
use maestro_kernel::{
    acquisition::{
        CaptureContext, CaptureEnvelope, Captures, DispatchRequest, Frontier, Handle, Item,
        ItemDisposition, LeaseRequest, Partitions, Receipts, Representation, SafeIdentity,
        StageItem, Transport,
    },
    artifact::Digest,
};
use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::io::{AsyncRead, AsyncWrite};

/// Verified reuse is not revalidation, and starts no fetch.
fn reuse<S: Captures + Receipts, T>(
    work: &mut CaptureWork<'_, '_, S, T>,
    item: &Item,
    report: &mut Report,
) -> Result<Option<StageItem>, Failure> {
    if item.capture.is_none() {
        return Ok(None);
    }
    let Some(observed) = work.observed.filter(|capture| capture.acknowledged) else {
        return Ok(None);
    };
    let handle = observed.handle;
    let envelope = &observed.envelope;
    work.keys
        .insert(item.id, super::sync_keys::observed_keys(envelope, item)?);
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
    if (item.capture.is_some() || work.observed.is_some()) && recheck(work, item).is_err() {
        report.pending.push(Entry::new(
            &item.request.fetch_identity,
            "current_resume_admission",
        ));
        return Ok(StageItem {
            item: item.id.to_string().parse().map_err(|_| storage())?,
            disposition: ItemDisposition::Pending,
            evidence: None,
        });
    }
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
    if let Some(handle) = work.observed.map(|capture| capture.handle) {
        return resume(work, item, handle, depth, report).await;
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
        carried_staging: Some((work.carried_staging, work.aggregate_bounds)),
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
    work.keys
        .insert(item.id, super::sync_keys::observed_keys(&envelope, item)?);
    stage.evidence = Some(handle);
    let acknowledged = interpret(work, (&context, handle), (&envelope, depth), report).await?;
    if acknowledged {
        work.captured_children
            .insert(item.request.fetch_identity.clone());
        stage.disposition = if work
            .previous
            .is_some_and(|previous| same_capture(previous, &envelope))
        {
            ItemDisposition::Unchanged
        } else {
            ItemDisposition::Accepted
        };
    }
    let mut entry = Entry::new(
        &item.request.fetch_identity,
        if acknowledged && work.previous.is_some() {
            if stage.disposition == ItemDisposition::Unchanged {
                "revalidated_unchanged"
            } else {
                "revalidated_changed"
            }
        } else if acknowledged {
            "captured"
        } else {
            "captured_discovery_pending"
        },
    );
    entry.observed_ms = Some(envelope.observed_ms);
    report.completed.push(entry);
    Ok(stage)
}
/// Re-lease verified prepared bytes for offline discovery, never another HTTP fetch.
async fn resume<S: Frontier + Captures + Partitions + Receipts, T>(
    work: &mut CaptureWork<'_, '_, S, T>,
    item: &Item,
    handle: Handle,
    depth: Option<u64>,
    report: &mut Report,
) -> Result<StageItem, Failure> {
    let now = SystemTime::now();
    let holder = work.source.receipt.attempt.to_string();
    let lease = work
        .source
        .store
        .lease(
            work.writer,
            item.id,
            DispatchRequest {
                lease: LeaseRequest {
                    holder: &holder,
                    now,
                    term: Duration::from_hours(1),
                },
                max_attempts: u32::try_from(
                    item.attempts
                        .saturating_add(work.accounting.limits().requests.get()),
                )
                .unwrap_or(u32::MAX),
            },
        )
        .map_err(|_| storage())?;
    let context = CaptureContext {
        writer: work.writer.clone(),
        item: lease,
        now,
    };
    let authorization = Digest::of(work.source.runtime.kernel_principal.as_bytes());
    let controls = Readiness(work.source.runtime.controls);
    let current = Current {
        policy: work.source.policy,
        controls: &controls,
        authority: work.source.runtime.authority,
        principal: work.source.principal.id,
        scope: work.source.scope,
        account: "public",
        authorization: &authorization,
        now,
        inputs: work.source.receipt.inputs,
    };
    let (envelope, _) = prepared(
        work.source.store,
        &current,
        (item, &context),
        handle,
        work.accounting.limits().dom_bytes.get(),
    )
    .map_err(|_| storage())?;
    let acknowledged = interpret(work, (&context, handle), (&envelope, depth), report).await?;
    work.keys
        .insert(item.id, super::sync_keys::observed_keys(&envelope, item)?);
    if acknowledged {
        work.captured_children
            .insert(item.request.fetch_identity.clone());
    }
    let stage = StageItem {
        item: item.id.to_string().parse().map_err(|_| storage())?,
        evidence: Some(handle),
        disposition: if acknowledged {
            ItemDisposition::Accepted
        } else {
            ItemDisposition::Pending
        },
    };
    let mut entry = Entry::new(
        &item.request.fetch_identity,
        "captured_earlier_not_revalidated",
    );
    entry.observed_ms = Some(envelope.observed_ms);
    report.completed.push(entry);
    Ok(stage)
}
/// Offline retained bytes reapply current policy/authority, without fetching robots.
fn recheck<S, T>(work: &CaptureWork<'_, '_, S, T>, item: &Item) -> Result<(), Refusal> {
    let authorization = Digest::of(work.source.runtime.kernel_principal.as_bytes());
    let controls = Readiness(work.source.runtime.controls);
    Current {
        policy: work.source.policy,
        controls: &controls,
        authority: work.source.runtime.authority,
        principal: work.source.principal.id,
        scope: work.source.scope,
        account: "public",
        authorization: &authorization,
        now: SystemTime::now(),
        inputs: work.source.receipt.inputs,
    }
    .check(item)
}
/// Known checkpoint depths resume; unrelated historical HTML remains held.
async fn interpret<S: Frontier + Captures + Partitions + Receipts, T>(
    work: &mut CaptureWork<'_, '_, S, T>,
    capture: (&CaptureContext, Handle),
    metadata: (&CaptureEnvelope, Option<u64>),
    report: &mut Report,
) -> Result<bool, Failure> {
    let (envelope, depth) = metadata;
    if let Some(depth) = depth {
        discovery(work, capture, envelope, depth, report).await
    } else if work.source.source.discovery.is_empty()
        || envelope
            .declared_media
            .as_deref()
            .is_some_and(|media| media != "text/html")
    {
        discovery(work, capture, envelope, 0, report).await
    } else {
        report.pending.push(Entry::new(
            envelope.requested.as_str(),
            "discovery_depth_unknown",
        ));
        Ok(false)
    }
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
        .checkpoint(
            work.aggregate_bounds,
            Usage {
                staging_bytes: work
                    .carried_staging
                    .checked_add(work.usage.staging_bytes)
                    .ok_or_else(storage)?,
                ..*work.usage
            },
        )
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
            max_attempts: u32::try_from(
                item.attempts
                    .saturating_add(work.accounting.limits().requests.get()),
            )
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
        observed_ms: u64::try_from(
            (work.source.runtime.clock)()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| storage())?
                .as_millis(),
        )
        .map_err(|_| storage())?,
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
