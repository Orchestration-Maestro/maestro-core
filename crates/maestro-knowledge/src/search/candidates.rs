//! Scoped prepared-input loading for the fused candidate IDs only.

use super::{
    candidate_enrichment::{self, Enriched, Settings},
    deadline,
    fusion::Fused,
    request::SearchConfiguration,
    rerank::Candidate,
    source_class::{self, SourceClassifier},
};
use maestro_kernel::{
    chunk_set::Chunk,
    generation::Generation,
    retrieval::{self, ReadControl, SearchRead},
    scope::ScopeSet,
    store::Database,
};
use std::{
    collections::{BTreeSet, HashMap},
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
    /// The earlier cutoff after which enrichment keeps chunk text.
    pub(super) context_deadline: Instant,
    /// Validated optional ranking policies.
    pub(super) configuration: SearchConfiguration,
    /// Query used only to exempt explicitly requested section classes.
    pub(super) query: String,
    /// The source classifier the source prior reads, when one is configured.
    pub(super) source_classes: Option<Arc<dyn SourceClassifier>>,
}

/// The candidates each soft prior penalizes.
#[derive(Debug, Default)]
pub(super) struct Penalized {
    /// Those whose section class the section prior penalizes.
    pub(super) section: BTreeSet<String>,
    /// Those whose source class the source prior penalizes.
    pub(super) source: BTreeSet<String>,
}

/// Loaded candidates and opt-in context diagnostics.
#[derive(Debug, Default)]
pub(super) struct Loaded {
    /// Prepared or expanded reranker inputs in fused order.
    pub(super) candidates: Vec<Candidate>,
    /// Source-loading wall time, including validation.
    pub(super) source_load_micros: u64,
    /// Chunks kept under bounded context: an oversized mandatory unit, an
    /// unavailable source, or the enrichment cutoff.
    pub(super) fallbacks: Vec<String>,
    /// Candidates classified for a soft penalty.
    pub(super) penalized: Penalized,
    /// Candidates whose enrichment was unavailable.
    pub(super) context_unavailable: usize,
}

impl Loaded {
    /// The candidates of `fused` with their `texts`, and what enrichment and
    /// the source classification found.
    fn new(
        fused: Vec<Fused>,
        texts: Vec<(&Chunk, String)>,
        enriched: Enriched,
        source: BTreeSet<String>,
    ) -> Self {
        Self {
            candidates: fused
                .into_iter()
                .zip(texts)
                .map(|(fused, (_, text))| Candidate { fused, text })
                .collect(),
            source_load_micros: enriched.micros,
            fallbacks: enriched.fallbacks,
            penalized: Penalized {
                section: enriched.penalized,
                source,
            },
            context_unavailable: enriched.unavailable,
        }
    }
}

/// Loads each fused chunk's strict UTF-8 prepared input in fused order.
pub(super) async fn load(database: Arc<Database>, request: Request) -> Result<Loaded, Failure> {
    let Request {
        generation,
        scopes,
        version,
        fused,
        expected_revisions,
        deadline,
        context_deadline,
        configuration,
        query,
        source_classes,
    } = request;
    if fused.is_empty() {
        return Ok(Loaded::default());
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
            deadline: deadline::std_deadline(deadline),
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
        let mut texts = Vec::with_capacity(fused.len());
        for fused in &fused {
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
            texts.push((chunk, text));
        }
        let enriched = candidate_enrichment::enrich(
            (&database, &scopes, &control),
            &Settings {
                configuration,
                query: &query,
                generation: &generation,
                deadline: deadline::std_deadline(context_deadline),
            },
            &mut texts,
        );
        let enrichment = ReadControl {
            deadline: control
                .deadline
                .min(deadline::std_deadline(context_deadline)),
            cancelled: control.cancelled.clone(),
        };
        let source = source_class::penalized(
            (&database, &scopes, &enrichment),
            source_classes.as_deref(),
            configuration.source_prior,
            texts
                .iter()
                .take(configuration.rerank_depth.get())
                .map(|(chunk, _)| *chunk),
        );
        check_control(&control)?;
        Ok(Loaded::new(fused, texts, enriched, source))
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
