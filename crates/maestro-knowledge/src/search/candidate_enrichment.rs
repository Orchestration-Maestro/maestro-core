//! Opt-in reranker context and section classes read from authoritative sources.
//!
//! Enrichment is best effort: a source that cannot be read or expanded, or
//! the enrichment cutoff, keeps the indexed chunk text and no penalty.

use super::{
    deadline,
    evidence::{self, Indexing, SourceCache},
    request::{CandidateContext, SearchConfiguration},
};
use maestro_canonicalization::{ChunkProfile, SourceSpan, chrome_spans};
use maestro_kernel::{
    chunk_set::Chunk, generation::Generation, retrieval::ReadControl, scope::ScopeSet,
    store::Database,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

/// Concurrent source loads for one search's enrichment.
const CONTEXT_LOAD_WORKERS: usize = 4;

/// What enrichment reads and until when.
pub(super) struct Settings<'a> {
    /// Validated optional ranking policies.
    pub(super) configuration: SearchConfiguration,
    /// Query used only to exempt explicitly requested section classes.
    pub(super) query: &'a str,
    /// The generation admitted before retrieval started.
    pub(super) generation: &'a Generation,
    /// The enrichment cutoff, before the rerank's own cutoff.
    pub(super) deadline: Instant,
}

/// Enrichment results for the candidates within the rerank depth.
#[derive(Debug, Default)]
pub(super) struct Enriched {
    /// Candidates that kept their indexed chunk under bounded context.
    pub(super) fallbacks: Vec<String>,
    /// Candidates classified for a soft penalty.
    pub(super) penalized: BTreeSet<String>,
    /// Candidates whose source was unreadable or unexpandable, or reached
    /// after the cutoff.
    pub(super) unavailable: usize,
    /// Revisions submitted for source loading in tests.
    #[cfg(test)]
    pub(super) requested_revisions: BTreeSet<String>,
    /// Source-loading and expansion wall time.
    pub(super) micros: u64,
}

/// Replaces each text within the rerank depth by its bounded section
/// context, and classifies its section, when configured.
pub(super) fn enrich(
    (database, scopes, control): (&Database, &ScopeSet, &ReadControl),
    settings: &Settings<'_>,
    candidates: &mut [(&Chunk, String)],
) -> Enriched {
    let configuration = settings.configuration;
    let max_bytes = match configuration.candidate_context {
        CandidateContext::BoundedSection { max_bytes } if configuration.rerank_enabled => {
            Some(max_bytes)
        }
        _ => None,
    };
    let prior = configuration.section_prior;
    let mut enriched = Enriched::default();
    if max_bytes.is_none() && !prior.is_active() {
        return enriched;
    }
    let started = Instant::now();
    let depth = configuration.rerank_depth.get().min(candidates.len());
    let candidates = candidates.get_mut(..depth).unwrap_or_default();
    let control = ReadControl {
        deadline: control.deadline.min(settings.deadline),
        clock: control.clock.clone(),
        cancelled: control.cancelled.clone(),
    };
    let profile = chunk_profile(database, scopes, settings.generation).unwrap_or_default();
    let mut chrome: BTreeMap<String, Option<Vec<SourceSpan>>> = BTreeMap::new();
    let mut cache = SourceCache::new(database, scopes, &control);
    let revisions = candidates
        .iter()
        .map(|(chunk, _)| chunk.revision_id.clone())
        .collect::<Vec<_>>();
    cache.load_available(&revisions, settings.generation, CONTEXT_LOAD_WORKERS);
    #[cfg(test)]
    {
        enriched.requested_revisions = cache.requested_revisions();
    }
    for (chunk, text) in candidates {
        let context = deadline::open(&control)
            .then(|| cache.get(&chunk.revision_id))
            .flatten()
            .and_then(|source| {
                let spans = chrome
                    .entry(chunk.revision_id.clone())
                    .or_insert_with(|| {
                        chrome_spans(&source.canonical, &source.markdown, profile).ok()
                    })
                    .as_deref()?;
                let indexing = Indexing {
                    profile,
                    chrome: spans,
                };
                let (path, expanded) =
                    evidence::context(source, chunk, max_bytes, indexing).ok()?;
                Some((format!("{} / {path}", source.document.source_ref), expanded))
            });
        let Some((path, expanded)) = context else {
            enriched.unavailable += 1;
            if max_bytes.is_some() {
                enriched.fallbacks.push(chunk.id.clone());
            }
            continue;
        };
        if prior.penalizes(settings.query, &path) {
            enriched.penalized.insert(chunk.id.clone());
        }
        if let Some(expanded) = expanded {
            *text = expanded;
        } else if max_bytes.is_some() {
            enriched.fallbacks.push(chunk.id.clone());
        }
    }
    enriched.micros = micros(started.elapsed());
    enriched
}

/// The chunking profile of the generation's chunk set, whose page chrome a
/// bounded context leaves out; none when it cannot be read or this build does
/// not know it, and the default profile, which leaves nothing out, applies.
fn chunk_profile(
    database: &Database,
    scopes: &ScopeSet,
    generation: &Generation,
) -> Option<ChunkProfile> {
    let set = database
        .chunk_set(scopes, &generation.chunk_set_id)
        .ok()
        .flatten()?;
    ChunkProfile::named(&set.chunk_profile)
}

/// Whole microseconds, saturating.
pub(super) fn micros(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX)
}
