//! Loads and caches authoritative source records for one assembly request.

use super::{
    families::{SectionOccurrence, section_occurrences},
    sections::SectionIndex,
    types::EvidenceError,
};
use maestro_canonicalization::{CanonicalDocument, SourceSpan, ValidationStatus};
use maestro_kernel::{
    document::{Disposition, Document, Outcome, Revision, RevisionStatus},
    evidence::Span,
    generation::Generation,
    retrieval::ReadControl,
    scope::ScopeSet,
    store::Database,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Instant,
};

/// Maximum number of scoped source-loading workers per assembly.
const MAX_SOURCE_LOAD_WORKERS: usize = 4;

/// One revision's authorized records and verified original source.
pub(crate) struct EvidenceSource {
    /// The immutable revision and its artifact identities.
    pub(crate) revision: Revision,
    /// The source document admitted by the read scopes.
    pub(crate) document: Document,
    /// The accepted disposition and its explicit source limitations.
    pub(crate) disposition: Disposition,
    /// The canonical representation structurally checked against original bytes.
    pub(crate) canonical: CanonicalDocument,
    /// The exact original UTF-8 Markdown.
    pub(crate) markdown: String,
    /// Validated source ranges and lexical section relationships.
    pub(crate) sections: SectionIndex,
    /// One-pass section path occurrences for family matching.
    pub(crate) section_occurrences: BTreeMap<String, SectionOccurrence>,
}

/// Request-local cache of source artifacts, keyed by their authorized revision.
/// ponytail: retain full source per request; cap bytes only if measurements show memory pressure.
pub(crate) struct SourceCache<'a> {
    /// Kernel store used for reads authorized by the admitted scopes.
    database: &'a Database,
    /// Immutable read-grant snapshot accepted with the request.
    scopes: &'a ScopeSet,
    /// Shared cancellation flag and inherited deadline.
    control: &'a ReadControl,
    /// Loaded source records keyed by authorized revision ID.
    sources: BTreeMap<String, EvidenceSource>,
    #[cfg(test)]
    load_counts: BTreeMap<String, ArtifactLoadCounts>,
}

#[cfg(test)]
#[derive(Default)]
struct ArtifactLoadCounts {
    canonical: usize,
    original: usize,
}

impl<'a> SourceCache<'a> {
    /// Starts an empty request-local cache over one admitted permission snapshot.
    pub(crate) fn new(
        database: &'a Database,
        scopes: &'a ScopeSet,
        control: &'a ReadControl,
    ) -> Self {
        Self {
            database,
            scopes,
            control,
            sources: BTreeMap::new(),
            #[cfg(test)]
            load_counts: BTreeMap::new(),
        }
    }

    /// Returns one verified source, loading both immutable artifacts at most once.
    pub(crate) fn load(
        &mut self,
        revision_id: &str,
        generation: &Generation,
    ) -> Result<&EvidenceSource, EvidenceError> {
        self.check()?;
        if !self.sources.contains_key(revision_id) {
            let source = self.load_source(revision_id, &generation.collection_id)?;
            self.sources.insert(revision_id.to_owned(), source);
        }
        let source = self
            .sources
            .get(revision_id)
            .ok_or(EvidenceError::WorkerFailed)?;
        self.check()?;
        Ok(source)
    }

    /// Loads unique revisions concurrently, merging successes and errors in input order.
    pub(super) fn load_many_with_workers(
        &mut self,
        revision_ids: &[String],
        generation: &Generation,
        worker_limit: usize,
    ) -> Result<(), EvidenceError> {
        let outcomes = self.load_concurrently(revision_ids, generation, worker_limit)?;
        let loaded = outcomes.into_iter().collect::<Result<Vec<_>, _>>()?;
        self.check()?;
        self.insert_loaded(loaded);
        self.check()
    }

    /// Loads unique revisions concurrently and keeps each one that loads:
    /// optional enrichment degrades per revision, while assembly still
    /// refuses a revision that fails its checks.
    pub(in crate::search) fn load_available(
        &mut self,
        revision_ids: &[String],
        generation: &Generation,
        worker_limit: usize,
    ) {
        if let Ok(outcomes) = self.load_concurrently(revision_ids, generation, worker_limit) {
            self.insert_loaded(outcomes.into_iter().flatten().collect());
        }
    }

    /// Runs the scoped workers over the revisions not yet cached, returning
    /// one outcome per revision in input order.
    fn load_concurrently(
        &self,
        revision_ids: &[String],
        generation: &Generation,
        worker_limit: usize,
    ) -> Result<Vec<Result<LoadedSource, EvidenceError>>, EvidenceError> {
        self.check()?;
        let mut seen = BTreeSet::new();
        let missing: Vec<_> = revision_ids
            .iter()
            .filter(|revision| {
                !self.sources.contains_key(revision.as_str()) && seen.insert(revision.as_str())
            })
            .cloned()
            .collect();
        if missing.is_empty() {
            return Ok(Vec::new());
        }

        let worker_count = worker_limit
            .clamp(1, MAX_SOURCE_LOAD_WORKERS)
            .min(missing.len());
        let next = AtomicUsize::new(0);
        let work = SourceLoadWork {
            database: self.database,
            scopes: self.scopes,
            control: self.control,
            missing: &missing,
            generation,
            next: &next,
        };
        let (worker_panicked, messages) = thread::scope(|scope| {
            let mut handles = Vec::with_capacity(worker_count);
            for _ in 0..worker_count {
                handles.push(spawn_source_worker(scope, &work));
            }
            let mut worker_panicked = false;
            let mut messages = Vec::with_capacity(missing.len());
            for handle in handles {
                match handle.join() {
                    Ok(mut worker_messages) => messages.append(&mut worker_messages),
                    Err(_) => worker_panicked = true,
                }
            }
            (worker_panicked, messages)
        });
        if worker_panicked {
            return Err(EvidenceError::WorkerFailed);
        }
        let mut outcomes: Vec<_> = (0..missing.len()).map(|_| None).collect();
        for (index, result) in messages {
            let Some(outcome) = outcomes.get_mut(index) else {
                return Err(EvidenceError::WorkerFailed);
            };
            if outcome.replace(result).is_some() {
                return Err(EvidenceError::WorkerFailed);
            }
        }
        outcomes
            .into_iter()
            .map(|outcome| outcome.ok_or(EvidenceError::WorkerFailed))
            .collect()
    }

    /// Caches loaded sources by revision.
    fn insert_loaded(&mut self, loaded: Vec<LoadedSource>) {
        #[cfg(test)]
        for (revision_id, source, counts) in loaded {
            self.sources.insert(revision_id.clone(), source);
            self.load_counts.insert(revision_id, counts);
        }
        #[cfg(not(test))]
        for (revision_id, source, ()) in loaded {
            self.sources.insert(revision_id, source);
        }
    }

    /// Returns an already-loaded authoritative revision without mutating the cache.
    pub(crate) fn get(&self, revision_id: &str) -> Option<&EvidenceSource> {
        self.sources.get(revision_id)
    }

    /// Rejects empty, reversed, out-of-range and non-boundary chunk spans.
    pub(crate) fn validate_chunk_span(
        source: &EvidenceSource,
        span: Span,
    ) -> Result<(), EvidenceError> {
        let source_span = SourceSpan {
            start: span.start,
            end: span.end,
        };
        if span.start >= span.end || !source_span.is_valid(&source.markdown) {
            return Err(integrity("candidate chunk has an invalid source span"));
        }
        Ok(())
    }

    /// Counts actual artifact reads in tests without adding a store abstraction.
    #[cfg(test)]
    pub(crate) fn load_counts(&self, revision_id: &str) -> Option<(usize, usize)> {
        self.load_counts
            .get(revision_id)
            .map(|counts| (counts.canonical, counts.original))
    }

    /// Loads and validates one revision's scoped records and exact artifacts.
    fn load_source(
        &mut self,
        revision_id: &str,
        collection_id: &str,
    ) -> Result<EvidenceSource, EvidenceError> {
        self.check()?;
        let revision = self
            .database
            .revision(self.scopes, revision_id)
            .map_err(EvidenceError::Records)?
            .ok_or(EvidenceError::NotVisible)?;
        self.check()?;
        let document = self
            .database
            .document(self.scopes, &revision.document_id)
            .map_err(EvidenceError::Records)?
            .ok_or(EvidenceError::NotVisible)?;
        self.check()?;
        let disposition = self
            .database
            .disposition(self.scopes, revision_id)
            .map_err(EvidenceError::Records)?
            .ok_or(EvidenceError::NotVisible)?;
        self.check()?;

        if revision.document_id != document.id || document.collection_id != collection_id {
            return Err(integrity("revision and document identity do not match"));
        }
        if revision.status == RevisionStatus::Failed
            || !matches!(
                disposition.outcome,
                Outcome::Accepted | Outcome::AcceptedWithWarnings
            )
        {
            return Err(integrity("evidence eligibility changed during assembly"));
        }

        self.check()?;
        #[cfg(test)]
        {
            self.load_counts
                .entry(revision_id.to_owned())
                .or_default()
                .canonical += 1;
        }
        let canonical_bytes = self
            .database
            .get(&revision.canonical_digest)
            .map_err(EvidenceError::Store)?;
        self.check()?;
        let canonical: CanonicalDocument = serde_json::from_slice(&canonical_bytes)
            .map_err(|_| integrity("canonical artifact is not valid JSON"))?;
        if canonical.document_id != document.id
            || canonical.revision_id != revision.id
            || canonical.content_hash != format!("sha256:{}", revision.original_digest.as_str())
            || canonical.original_markdown_reference.content_hash != canonical.content_hash
            || canonical.validation_status == ValidationStatus::Failed
        {
            return Err(integrity(
                "canonical artifact identity or status is invalid",
            ));
        }

        self.check()?;
        #[cfg(test)]
        {
            self.load_counts
                .entry(revision_id.to_owned())
                .or_default()
                .original += 1;
        }
        let original_bytes = self
            .database
            .get(&revision.original_digest)
            .map_err(EvidenceError::Store)?;
        self.check()?;
        let markdown =
            String::from_utf8(original_bytes).map_err(|_| integrity("source is not UTF-8"))?;
        if canonical.original_markdown_reference.byte_length != markdown.len() {
            return Err(integrity(
                "canonical source length does not match its artifact",
            ));
        }
        let sections = SectionIndex::new(&canonical, &markdown)
            .map_err(|_| integrity("canonical source spans or links are invalid"))?;
        let section_occurrences = section_occurrences(&canonical)
            .map_err(|_| integrity("canonical section identities are invalid"))?;
        self.check()?;
        Ok(EvidenceSource {
            revision,
            document,
            disposition,
            canonical,
            markdown,
            sections,
            section_occurrences,
        })
    }

    /// Stops before and after blocking work when cancellation or expiry fires.
    fn check(&self) -> Result<(), EvidenceError> {
        if self.control.cancelled.load(Ordering::Relaxed) || Instant::now() >= self.control.deadline
        {
            Err(EvidenceError::TimedOut)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
/// Keeps artifact-read counters with each loaded source in tests.
type LoadedSource = (String, EvidenceSource, ArtifactLoadCounts);
#[cfg(not(test))]
/// Identifies a successfully loaded source in production.
type LoadedSource = (String, EvidenceSource, ());

/// Immutable inputs shared by scoped source-loading workers.
struct SourceLoadWork<'a> {
    /// Store used for scoped source reads.
    database: &'a Database,
    /// Permission snapshot used for every read.
    scopes: &'a ScopeSet,
    /// Shared cancellation flag and deadline.
    control: &'a ReadControl,
    /// Unique revision IDs not already cached.
    missing: &'a [String],
    /// Generation whose immutable source records are loaded.
    generation: &'a Generation,
    /// Atomic cursor assigning revision IDs to workers.
    next: &'a AtomicUsize,
}

/// Starts one scoped worker over the shared source-load queue.
fn spawn_source_worker<'scope, 'env: 'scope>(
    scope: &'scope thread::Scope<'scope, 'env>,
    work: &'scope SourceLoadWork<'env>,
) -> thread::ScopedJoinHandle<'scope, Vec<(usize, Result<LoadedSource, EvidenceError>)>> {
    scope.spawn(move || load_many_worker(work))
}

/// Loads indexed revisions using one request-local cache on each worker.
fn load_many_worker(
    work: &SourceLoadWork<'_>,
) -> Vec<(usize, Result<LoadedSource, EvidenceError>)> {
    let mut cache = SourceCache::new(work.database, work.scopes, work.control);
    let mut messages = Vec::new();
    loop {
        let index = work.next.fetch_add(1, Ordering::Relaxed);
        let Some(revision_id) = work.missing.get(index) else {
            break;
        };
        let result = match cache.load(revision_id, work.generation) {
            Ok(_) => {
                let source = cache
                    .sources
                    .remove(revision_id)
                    .ok_or(EvidenceError::WorkerFailed);
                #[cfg(test)]
                let loaded = source.and_then(|source| {
                    let counts = cache
                        .load_counts
                        .remove(revision_id)
                        .ok_or(EvidenceError::WorkerFailed)?;
                    Ok((revision_id.clone(), source, counts))
                });
                #[cfg(not(test))]
                let loaded = source.map(|source| (revision_id.clone(), source, ()));
                loaded
            }
            Err(error) => Err(error),
        };
        messages.push((index, result));
    }
    messages
}

/// Keeps integrity diagnostics independent of source text and record IDs.
fn integrity(reason: &str) -> EvidenceError {
    EvidenceError::Integrity(reason.to_owned())
}
