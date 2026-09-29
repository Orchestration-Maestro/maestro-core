//! Exact identifier search from Qdrant payloads and the kernel identifier index.

use super::{
    identifier_cursor::advances,
    identifier_payload::{invalid_answer, payload_hit},
    outcome::{DroppedIdentifier, IdentifierOutcome, RouteOutcome},
    results::ScoredChunk,
};
use crate::{
    index::{ProjectionCursor, ProjectionFilter, RetrievalProjectionPort},
    query::{PROFILE, Understood},
    search::{
        deadline::{self, DEADLINE_EXCEEDED, DISABLED_BY_CONFIGURATION},
        filter::query_filter,
        query::Query,
    },
};
use maestro_kernel::{
    evidence::RouteStatus,
    retrieval::{self, ReadControl, SearchRead},
    scope::ScopeSet,
    store::Database,
};
use std::{
    collections::HashSet,
    sync::{Arc, atomic::Ordering},
};
use tokio::time::{self, Instant};

/// How the identifier route runs, as the search configuration sets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::search) enum IdentifierMode {
    /// The route does not run.
    Off,
    /// Both legs run concurrently on every identifier.
    On,
    /// The kernel leg runs first, and the payload leg only on the
    /// identifiers the kernel leg did not skip as too common.
    Guarded,
}

/// Why the noise guard drops an identifier that matches more chunks than
/// the route can rank.
const TOO_COMMON: &str = "identifier too common";

/// Finds identifiers in a pinned generation using exact payloads and indexed scoped rows.
///
/// Plain words are left to lexical search; this route indexes only identifiers
/// recognized by the shared extractor.
/// The two legs run concurrently. A completed leg's hits survive failure or
/// timeout in the other; common kernel identifiers are reported as degraded.
/// One Identifier route remains one fusion vote.
pub async fn search_identifiers<R: RetrievalProjectionPort>(
    query: &Query<'_, R>,
    database: Arc<Database>,
    understood: &Understood,
    deadline: Instant,
) -> RouteOutcome {
    search_identifiers_as(IdentifierMode::On, query, database, understood, deadline)
        .await
        .route
}

/// [`search_identifiers`] with the noise guard: an identifier the kernel
/// leg skips as too common is dropped from the payload leg too, and the
/// route is unavailable only when a leg fails or every identifier is dropped.
pub async fn search_identifiers_guarded<R: RetrievalProjectionPort>(
    query: &Query<'_, R>,
    database: Arc<Database>,
    understood: &Understood,
    deadline: Instant,
) -> IdentifierOutcome {
    search_identifiers_as(
        IdentifierMode::Guarded,
        query,
        database,
        understood,
        deadline,
    )
    .await
}

/// Executes identifier search as `mode` says.
pub(in crate::search) async fn search_identifiers_as<R: RetrievalProjectionPort>(
    mode: IdentifierMode,
    query: &Query<'_, R>,
    database: Arc<Database>,
    understood: &Understood,
    deadline: Instant,
) -> IdentifierOutcome {
    let route = |route| IdentifierOutcome {
        route,
        dropped: Vec::new(),
    };
    if mode == IdentifierMode::Off {
        return route(unavailable(Vec::new(), DISABLED_BY_CONFIGURATION));
    }
    let identifiers = unique_identifiers(understood);
    if identifiers.is_empty() || query.limit == 0 || query.scopes.is_empty() {
        return route(ok(Vec::new()));
    }
    if identifiers.len() > 64 {
        return route(unavailable(
            Vec::new(),
            "kernel: too many identifier values",
        ));
    }
    let limit = query.limit.min(20);
    if Instant::now() >= deadline {
        return route(unavailable(Vec::new(), DEADLINE_EXCEEDED));
    }
    if let Err(reason) = ready_projection(
        database.clone(),
        query.scopes.clone(),
        query.generation.id,
        deadline,
    )
    .await
    {
        return route(unavailable(Vec::new(), &reason));
    }

    let (payload, kernel) = if mode == IdentifierMode::Guarded {
        guarded_legs(query, database, &identifiers, limit, deadline).await
    } else {
        tokio::join!(
            payload_leg(query, &identifiers, limit, deadline),
            kernel_leg(query, database, &identifiers, limit, deadline),
        )
    };
    if matches!((&payload, &kernel), (Err(payload), Err(kernel))
        if payload == DEADLINE_EXCEEDED && kernel == DEADLINE_EXCEEDED)
    {
        return route(unavailable(Vec::new(), DEADLINE_EXCEEDED));
    }
    merged(payload, kernel, mode, identifiers.len(), limit)
}

/// Runs the kernel leg, then the payload leg on the identifiers the kernel
/// leg did not skip as too common; when it skipped them all, the payload
/// leg does not run.
async fn guarded_legs<R: RetrievalProjectionPort>(
    query: &Query<'_, R>,
    database: Arc<Database>,
    identifiers: &[String],
    limit: usize,
    deadline: Instant,
) -> (
    Result<Vec<ScoredChunk>, String>,
    Result<KernelOutcome, String>,
) {
    let kernel = kernel_leg(query, database, identifiers, limit, deadline).await;
    let kept = match &kernel {
        Ok(kernel) => identifiers
            .iter()
            .filter(|identifier| !kernel.too_common.contains(identifier))
            .cloned()
            .collect(),
        Err(_) => identifiers.to_vec(),
    };
    let payload = if kept.is_empty() {
        Ok(Vec::new())
    } else {
        payload_leg(query, &kept, limit, deadline).await
    };
    (payload, kernel)
}

/// Merges the legs' hits, payload first, and states why the route is
/// degraded, if it is; guarded, the too-common identifiers are dropped and
/// degrade the route only when they are all of its `requested` identifiers.
fn merged(
    payload: Result<Vec<ScoredChunk>, String>,
    kernel: Result<KernelOutcome, String>,
    mode: IdentifierMode,
    requested: usize,
    limit: usize,
) -> IdentifierOutcome {
    let mut hits = Vec::new();
    let mut seen = HashSet::new();
    let mut reasons = Vec::new();
    let mut dropped = Vec::new();
    match payload {
        Ok(payload) => extend_unique(&mut hits, &mut seen, payload, limit),
        Err(reason) => reasons.push(format!("payload: {}", nonblank(&reason, "unavailable"))),
    }
    match kernel {
        Ok(kernel) if mode == IdentifierMode::Guarded => {
            extend_unique(&mut hits, &mut seen, kernel.hits, limit);
            if kernel.too_common.len() == requested {
                reasons.push(all_too_common(&kernel.too_common));
            }
            dropped = kernel
                .too_common
                .into_iter()
                .map(|identifier| DroppedIdentifier {
                    identifier,
                    reason: TOO_COMMON.to_owned(),
                })
                .collect();
        }
        Ok(kernel) => {
            extend_unique(&mut hits, &mut seen, kernel.hits, limit);
            if !kernel.too_common.is_empty() {
                reasons.push(format!("kernel: {TOO_COMMON}"));
            }
        }
        Err(reason) => reasons.push(format!("kernel: {}", nonblank(&reason, "unavailable"))),
    }
    let route = if reasons.is_empty() {
        ok(hits)
    } else {
        unavailable(hits, &reasons.join("; "))
    };
    IdentifierOutcome { route, dropped }
}

/// The reason a guarded route gives when it dropped every identifier as
/// too common, naming them.
fn all_too_common(identifiers: &[String]) -> String {
    let quoted: Vec<_> = identifiers
        .iter()
        .map(|identifier| format!("{identifier:?}"))
        .collect();
    format!("kernel: {TOO_COMMON}: {}", quoted.join(", "))
}

/// Deduplicates query identifier texts in their first-seen family order.
fn unique_identifiers(understood: &Understood) -> Vec<String> {
    let mut seen = HashSet::new();
    understood
        .identifiers
        .iter()
        .filter(|identifier| seen.insert(identifier.text.as_str()))
        .map(|identifier| identifier.text.clone())
        .collect()
}

/// Checks readiness before starting either backend leg.
async fn ready_projection(
    database: Arc<Database>,
    scopes: ScopeSet,
    generation_id: i64,
    deadline: Instant,
) -> Result<(), String> {
    match deadline::run_blocking(deadline, move |cancelled| {
        if cancelled.load(Ordering::Relaxed) {
            return Err(retrieval::Error::Cancelled);
        }
        let projection = database
            .generation_search(&scopes, generation_id)?
            .filter(|projection| projection.ready)
            .ok_or(retrieval::Error::ProjectionMissing)?;
        if projection.identifier_profile != PROFILE {
            return Err(retrieval::Error::ProfileMismatch {
                expected: PROFILE.to_owned(),
                found: projection.identifier_profile,
            });
        }
        Ok(())
    })
    .await
    {
        Ok(result) => result.map_err(|error| retrieval_reason(&error)),
        Err(deadline::BlockingFailure::TimedOut) => Err(DEADLINE_EXCEEDED.to_owned()),
        Err(deadline::BlockingFailure::WorkerFailed) => {
            Err("search projection check failed".to_owned())
        }
    }
}

/// Searches filtered payload pages in the pinned physical collection.
async fn payload_leg<R: RetrievalProjectionPort>(
    query: &Query<'_, R>,
    identifiers: &[String],
    limit: usize,
    deadline: Instant,
) -> Result<Vec<ScoredChunk>, String> {
    if Instant::now() >= deadline {
        return Err(DEADLINE_EXCEEDED.to_owned());
    }
    let filter = ProjectionFilter::All(vec![
        query_filter(query.scopes, query.version),
        ProjectionFilter::ExactString {
            field: "identifier_profile".to_owned(),
            value: PROFILE.to_owned(),
        },
        ProjectionFilter::AnyString {
            field: "identifiers".to_owned(),
            values: identifiers.to_vec(),
        },
    ]);
    let mut offset: Option<ProjectionCursor> = None;
    let candidate_limit = limit.saturating_mul(2);
    let mut hits = Vec::new();
    let mut seen = HashSet::new();
    loop {
        if Instant::now() >= deadline {
            return Err(DEADLINE_EXCEEDED.to_owned());
        }
        let page = time::timeout_at(
            deadline,
            query
                .qdrant
                .scroll(&query.collection(), filter.clone(), offset.clone()),
        )
        .await
        .map_err(|_| DEADLINE_EXCEEDED.to_owned())?
        .map_err(|_| "Qdrant payload search failed".to_owned())?;
        for point in page.points {
            let hit = payload_hit(&point)
                .map_err(|_| "Qdrant returned an invalid search payload".to_owned())?;
            if !seen.insert(hit.chunk_id.clone()) {
                continue;
            }
            hits.push(hit);
            if hits.len() == candidate_limit {
                return Ok(order_payload_hits(hits, limit));
            }
        }
        let Some(next) = page.next else {
            return Ok(order_payload_hits(hits, limit));
        };
        if !advances(offset.as_ref(), &next) {
            return Err(invalid_answer("Qdrant scroll pagination did not advance").to_string());
        }
        offset = Some(next);
    }
}

/// Sorts the bounded scroll prefix by route score and stable chunk ID.
fn order_payload_hits(hits: Vec<ScoredChunk>, limit: usize) -> Vec<ScoredChunk> {
    super::results::rank(hits, limit)
}

/// The exact kernel hits and the high-frequency identifiers it skipped.
struct KernelOutcome {
    /// Ranked chunks returned by the kernel identifier leg.
    hits: Vec<ScoredChunk>,
    /// The identifiers the kernel skipped as above the route's fetch limit.
    too_common: Vec<String>,
}

/// Reads an exact, scope-filtered kernel leg on a cancellable blocking worker.
async fn kernel_leg<R: RetrievalProjectionPort>(
    query: &Query<'_, R>,
    database: Arc<Database>,
    identifiers: &[String],
    limit: usize,
    deadline: Instant,
) -> Result<KernelOutcome, String> {
    let generation = query.generation.clone();
    let scopes = query.scopes.clone();
    let version = query.version.map(str::to_owned);
    let identifiers = identifiers.to_vec();
    match deadline::run_blocking(deadline, move |cancelled| {
        let control = ReadControl {
            deadline: deadline::std_deadline(deadline),
            cancelled,
        };
        let read = SearchRead {
            generation: &generation,
            scopes: &scopes,
            version: version.as_deref(),
            control: &control,
        };
        database.identifier_hits(&read, &identifiers, limit)
    })
    .await
    {
        Ok(Ok(result)) => Ok(KernelOutcome {
            hits: result
                .hits
                .into_iter()
                .map(|hit| ScoredChunk {
                    chunk_id: hit.chunk_id,
                    revision_id: hit.revision_id,
                    score: 1.0,
                })
                .collect(),
            too_common: result.too_common,
        }),
        Ok(Err(error)) => Err(retrieval_reason(&error)),
        Err(deadline::BlockingFailure::TimedOut) => Err(DEADLINE_EXCEEDED.to_owned()),
        Err(deadline::BlockingFailure::WorkerFailed) => {
            Err("kernel identifier search failed".to_owned())
        }
    }
}

/// Maps kernel storage failures to safe, stable public route reasons.
fn retrieval_reason(error: &retrieval::Error) -> String {
    match error {
        retrieval::Error::ProjectionMissing => {
            "search projection missing; publish a new generation".to_owned()
        }
        retrieval::Error::ProfileMismatch { .. } => {
            "search projection profile mismatch; publish a new generation".to_owned()
        }
        retrieval::Error::TimedOut | retrieval::Error::Cancelled => DEADLINE_EXCEEDED.to_owned(),
        retrieval::Error::UnknownOrInaccessible => {
            "generation is unavailable in the granted scope".to_owned()
        }
        retrieval::Error::Store(_)
        | retrieval::Error::InvalidInput(_)
        | retrieval::Error::InputConflict
        | retrieval::Error::MembershipConflict
        | retrieval::Error::TooLarge => "kernel identifier search failed".to_owned(),
    }
}

/// Adds the hits from one completed leg without disturbing payload-first order.
fn extend_unique(
    hits: &mut Vec<ScoredChunk>,
    seen: &mut HashSet<String>,
    incoming: Vec<ScoredChunk>,
    limit: usize,
) {
    for hit in incoming {
        if hits.len() >= limit {
            break;
        }
        if seen.insert(hit.chunk_id.clone()) {
            hits.push(hit);
        }
    }
}

/// Creates a healthy route outcome.
fn ok(hits: Vec<ScoredChunk>) -> RouteOutcome {
    RouteOutcome {
        hits,
        status: RouteStatus::Ok,
    }
}

/// Creates an unavailable route outcome with a nonblank diagnostic.
fn unavailable(hits: Vec<ScoredChunk>, reason: &str) -> RouteOutcome {
    RouteOutcome {
        hits,
        status: RouteStatus::Unavailable(nonblank(reason, "identifier route unavailable")),
    }
}

/// Replaces a blank failure reason with a stable nonblank fallback.
fn nonblank(reason: &str, fallback: &str) -> String {
    let reason = reason.trim();
    if reason.is_empty() {
        fallback.to_owned()
    } else {
        reason.to_owned()
    }
}
