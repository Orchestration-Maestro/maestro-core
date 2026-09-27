//! Scoped prepared-input loading for the fused candidate IDs only.

use super::{deadline, fusion::Fused, rerank::Candidate};
use maestro_kernel::{
    generation::Generation,
    retrieval::{self, ReadControl, SearchRead},
    scope::ScopeSet,
    store::Database,
};
use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
    time::Instant as StdInstant,
};
use tokio::time::Instant;

/// A candidate-loading outcome distinguishes deadline degradation from unsafe data.
#[derive(Debug)]
pub(super) enum Failure {
    /// Prepared text could not be loaded exactly and within scope.
    EvidenceLoad,
    /// The worker reached the caller's absolute work cutoff.
    TimedOut,
    /// A controlled kernel read failed for a reason other than missing evidence.
    Kernel(retrieval::Error),
    /// The blocking worker could not be joined.
    WorkerFailed,
}

/// The one pinned, authorized candidate batch and its absolute cutoff.
pub(super) struct Request {
    /// The generation admitted before retrieval started.
    pub(super) generation: Generation,
    /// The immutable scopes used by every route.
    pub(super) scopes: ScopeSet,
    /// The exact effective version filter.
    pub(super) version: Option<String>,
    /// The single fused list in its RRF order.
    pub(super) fused: Vec<Fused>,
    /// Every route revision that must agree with each kernel chunk.
    pub(super) expected_revisions: HashMap<String, Vec<String>>,
    /// The shared text-loading and reranking cutoff.
    pub(super) deadline: Instant,
}

/// Loads each fused chunk's strict UTF-8 prepared input in fused order.
pub(super) async fn load(
    database: Arc<Database>,
    request: Request,
) -> Result<Vec<Candidate>, Failure> {
    let Request {
        generation,
        scopes,
        version,
        fused,
        expected_revisions,
        deadline,
    } = request;
    if fused.is_empty() {
        return Ok(Vec::new());
    }
    if Instant::now() >= deadline {
        return Err(Failure::TimedOut);
    }
    let ids = fused
        .iter()
        .map(|candidate| candidate.chunk_id.clone())
        .collect::<Vec<_>>();
    deadline::run_blocking(deadline, move |cancelled| {
        let control = ReadControl {
            deadline: deadline.into_std(),
            cancelled,
        };
        let read = SearchRead {
            generation: &generation,
            scopes: &scopes,
            version: version.as_deref(),
            control: &control,
        };
        let chunks = database.search_chunks(&read, &ids).map_err(classify_read)?;
        let chunks = chunks
            .into_iter()
            .map(|chunk| (chunk.id.clone(), chunk))
            .collect::<HashMap<_, _>>();
        let mut candidates = Vec::with_capacity(fused.len());
        for fused in fused {
            check_control(&control)?;
            let chunk = chunks.get(&fused.chunk_id).ok_or(Failure::EvidenceLoad)?;
            if !expected_revisions
                .get(&fused.chunk_id)
                .is_some_and(|revisions| {
                    revisions
                        .iter()
                        .all(|revision| revision == &chunk.revision_id)
                })
            {
                return Err(Failure::EvidenceLoad);
            }
            let bytes = database
                .get(&chunk.digest)
                .map_err(|_| Failure::EvidenceLoad)?;
            check_control(&control)?;
            let text = String::from_utf8(bytes).map_err(|_| Failure::EvidenceLoad)?;
            candidates.push(Candidate { fused, text });
        }
        check_control(&control)?;
        Ok(candidates)
    })
    .await
    .map_err(|error| match error {
        deadline::BlockingFailure::TimedOut => Failure::TimedOut,
        deadline::BlockingFailure::WorkerFailed => Failure::WorkerFailed,
    })?
}

/// Maps a controlled SQLite read timeout separately from unsafe/missing data.
pub(super) fn classify_read(error: retrieval::Error) -> Failure {
    match error {
        retrieval::Error::TimedOut | retrieval::Error::Cancelled => Failure::TimedOut,
        error @ retrieval::Error::Store(_) => Failure::Kernel(error),
        retrieval::Error::UnknownOrInaccessible
        | retrieval::Error::ProjectionMissing
        | retrieval::Error::ProfileMismatch { .. }
        | retrieval::Error::InvalidInput(_)
        | retrieval::Error::InputConflict
        | retrieval::Error::MembershipConflict
        | retrieval::Error::TooLarge => Failure::EvidenceLoad,
    }
}

/// Stops artifact reads at cancellation or the absolute deadline.
pub(super) fn check_control(control: &ReadControl) -> Result<(), Failure> {
    if control.cancelled.load(Ordering::Relaxed) || StdInstant::now() >= control.deadline {
        Err(Failure::TimedOut)
    } else {
        Ok(())
    }
}
