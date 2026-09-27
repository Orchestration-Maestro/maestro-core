//! Loads the pinned candidate pool and drives source selection.

use super::super::super::request::EvidenceInput;
use super::super::conflicts::emit_conflicts;
use super::super::{
    budget::{self, CounterInfo},
    conflicts::detect::{ConflictFinding, detect_conflicts},
    selection::{self, SelectionBudget, SelectionCandidate, SelectionResult},
    source::SourceCache,
    spans::{SeedSpan, union_seed_spans},
    types::{EvidenceCounter, EvidenceError},
};
use super::{
    candidates::{
        candidate_texts, conflict_required_spans, conflict_sources, make_candidate_data,
        selected_originals,
    },
    ledger::{DuplicateLedger, duplicate_ledger},
    types::{BundleParts, CandidateData, LoadedCandidate},
};
use maestro_kernel::{
    document::Revision,
    evidence::Bundle,
    generation::{Generation, GenerationState},
    retrieval::{Error as RetrievalError, ReadControl, SearchRead},
    store::Database,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::Ordering,
    time::Instant,
};

/// A request worker holding its pinned authority and source cache.
pub(super) struct AssemblyWorker<'a> {
    /// The kernel database for scoped reads.
    pub(super) database: &'a Database,
    /// The exact T029c handoff accepted at admission.
    pub(super) input: &'a EvidenceInput,
    /// The answerer-bound counter selected by the caller.
    pub(super) counter: &'a EvidenceCounter,
    /// The absolute deadline and caller cancellation flag.
    pub(super) control: &'a ReadControl,
    /// The re-read immutable generation identity.
    pub(super) generation: Generation,
    /// The completed set's validated duplicate manifest.
    pub(super) ledger: DuplicateLedger,
    /// The request-local cache of authorized source artifacts.
    pub(super) sources: SourceCache<'a>,
    /// Whether any named candidate was omitted by scoped current eligibility.
    pub(super) unresolved_candidate: bool,
}

/// Runs the bounded request on a blocking thread, away from async runtimes.
pub(super) fn assemble_blocking(
    database: &Database,
    input: &EvidenceInput,
    counter: &EvidenceCounter,
    control: &ReadControl,
) -> Result<Bundle, EvidenceError> {
    check(control)?;
    let counter_info = budget::counter_info(counter).map_err(EvidenceError::from)?;
    budget::verify_counter(counter, &counter_info).map_err(EvidenceError::from)?;
    check(control)?;
    let visible = database
        .visible(&input.principal)
        .map_err(EvidenceError::Store)?;
    check(control)?;
    if visible != *input.scopes {
        return Err(EvidenceError::PermissionsChanged);
    }
    let generation = database
        .generation(&input.scopes, input.generation.id)
        .map_err(EvidenceError::Generation)?
        .ok_or(EvidenceError::NotVisible)?;
    check(control)?;
    validate_generation(&input.generation, &generation)?;
    let ledger = duplicate_ledger(database, &input.scopes, &generation.chunk_set_id, control)
        .map_err(EvidenceError::from)?;
    check(control)?;

    let mut worker = AssemblyWorker {
        sources: SourceCache::new(database, &input.scopes, control),
        database,
        input,
        counter,
        control,
        generation,
        ledger,
        unresolved_candidate: false,
    };
    worker.assemble(counter_info)
}

impl AssemblyWorker<'_> {
    /// Resolves and groups source spans, then runs deterministic selection.
    fn assemble(&mut self, counter_info: CounterInfo) -> Result<Bundle, EvidenceError> {
        check(self.control)?;
        let loaded = self.load_candidates()?;
        let near_groups = self.near_duplicate_groups(&loaded)?;
        let seeds = self.load_seed_spans(&loaded)?;
        let unions =
            union_seed_spans(seeds).map_err(|_| integrity("candidate span unions are invalid"))?;
        let mut candidates = self.candidate_data(unions, &near_groups)?;
        let all_seeds = candidates
            .iter()
            .flat_map(|candidate| candidate.seeds.seeds.iter().cloned())
            .collect::<Vec<_>>();
        let proposed_texts = candidate_texts(&candidates, &self.sources)?;
        let conflict_sources = conflict_sources(&candidates, &self.sources)?;
        let findings = detect_conflicts(&conflict_sources)
            .map_err(|_| integrity("canonical conflict observations are invalid"))?;
        drop(conflict_sources);
        let collapse = super::candidates::collapse_candidate_versions(
            self.input,
            &mut candidates,
            &findings,
            &self.sources,
        )?;
        let conflict_sources = super::candidates::conflict_sources(&candidates, &self.sources)?;
        let (selection, indices) = self.select(&candidates, &findings, &collapse, &counter_info)?;
        let selected = selected_originals(&selection.selected_candidates, &indices)?;
        let emission = emit_conflicts(&findings, &conflict_sources, &selected, &selection.passages)
            .map_err(|_| integrity("selected conflict provenance is invalid"))?;
        self.finish(
            counter_info,
            BundleParts {
                passages: selection.passages,
                seeds: all_seeds,
                candidate_texts: proposed_texts,
                emission,
                omissions: selection.omissions,
                latest_undetermined: collapse.latest_undetermined,
            },
        )
    }

    /// Loads only ranked IDs through T029c's controlled pinned-set reader.
    fn load_candidates(&mut self) -> Result<Vec<LoadedCandidate>, EvidenceError> {
        check(self.control)?;
        let ids = self
            .input
            .ranked
            .iter()
            .map(|ranked| ranked.candidate.fused.chunk_id.clone())
            .collect::<Vec<_>>();
        let chunks = self
            .database
            .search_chunks(
                &SearchRead {
                    generation: &self.generation,
                    scopes: self.input.scopes.as_ref(),
                    version: self.input.version.as_deref(),
                    control: self.control,
                },
                &ids,
            )
            .map_err(map_kernel_error)?;
        check(self.control)?;
        let mut by_id = BTreeMap::new();
        for chunk in chunks {
            if by_id.insert(chunk.id.clone(), chunk).is_some() {
                return Err(integrity("scoped chunk reader returned a duplicate ID"));
            }
        }
        let mut loaded = Vec::with_capacity(by_id.len());
        for (input_position, ranked) in self.input.ranked.iter().enumerate() {
            let id = &ranked.candidate.fused.chunk_id;
            let Some(chunk) = by_id.remove(id) else {
                self.unresolved_candidate = true;
                continue;
            };
            if !self.ledger.revisions.contains(&chunk.revision_id)
                || self.ledger.duplicates.contains_key(&chunk.revision_id)
            {
                return Err(integrity("loaded candidate is outside the manifest ledger"));
            }
            loaded.push(LoadedCandidate {
                chunk,
                input_position,
                score: ranked.score,
                routes: ranked.candidate.fused.ranks.keys().copied().collect(),
            });
        }
        if !by_id.is_empty() {
            return Err(integrity("scoped chunk reader returned an unrequested ID"));
        }
        Ok(loaded)
    }

    /// Intersects cumulative near-duplicate rows with all three pinned ledgers.
    fn near_duplicate_groups(
        &self,
        loaded: &[LoadedCandidate],
    ) -> Result<BTreeMap<String, BTreeSet<String>>, EvidenceError> {
        let revisions: BTreeSet<_> = loaded
            .iter()
            .map(|candidate| candidate.chunk.revision_id.as_str())
            .collect();
        let loaded_revisions: BTreeSet<_> = revisions.iter().copied().collect();
        let mut groups = BTreeMap::<String, BTreeSet<String>>::new();
        for revision in revisions {
            check(self.control)?;
            let members = self
                .database
                .near_duplicates(self.input.scopes.as_ref(), revision)
                .map_err(EvidenceError::Records)?;
            check(self.control)?;
            for member in members.into_iter().filter(|member| {
                self.ledger.near_duplicate_groups.contains(&member.group_id)
                    && self.ledger.revisions.contains(&member.revision_id)
                    && loaded_revisions.contains(member.revision_id.as_str())
            }) {
                groups
                    .entry(member.revision_id)
                    .or_default()
                    .insert(member.group_id);
            }
        }
        Ok(groups)
    }

    /// Validates every loaded chunk span against its cached original Markdown.
    fn load_seed_spans(
        &mut self,
        loaded: &[LoadedCandidate],
    ) -> Result<Vec<SeedSpan>, EvidenceError> {
        let mut seeds = Vec::with_capacity(loaded.len());
        for candidate in loaded {
            check(self.control)?;
            let source = self
                .sources
                .load(&candidate.chunk.revision_id, &self.generation)?;
            if source.revision.id != candidate.chunk.revision_id {
                return Err(integrity("chunk and revision identities do not match"));
            }
            SourceCache::validate_chunk_span(source, candidate.chunk.span)?;
            if let Some(version) = self.input.version.as_deref()
                && metadata_string(&source.revision, "version").as_deref() != Some(version)
            {
                return Err(integrity(
                    "loaded candidate does not match its version filter",
                ));
            }
            seeds.push(SeedSpan {
                chunk_id: candidate.chunk.id.clone(),
                revision_id: candidate.chunk.revision_id.clone(),
                section_id: candidate.chunk.section_id.clone(),
                span: candidate.chunk.span,
                input_position: candidate.input_position,
                score: candidate.score,
                routes: candidate.routes.clone(),
            });
        }
        Ok(seeds)
    }

    /// Expands touching seeds into candidates using the cached source structures.
    fn candidate_data(
        &self,
        unions: Vec<super::super::spans::SpanUnion>,
        near_groups: &BTreeMap<String, BTreeSet<String>>,
    ) -> Result<Vec<CandidateData>, EvidenceError> {
        unions
            .into_iter()
            .map(|seeds| make_candidate_data(seeds, &self.sources, near_groups))
            .collect()
    }

    /// Selects conflict-atomic candidates and their source-safe windows.
    fn select(
        &self,
        candidates: &[CandidateData],
        findings: &[ConflictFinding],
        collapse: &super::super::versions::VersionCollapse,
        counter_info: &CounterInfo,
    ) -> Result<(SelectionResult, Vec<usize>), EvidenceError> {
        let indices: Vec<_> = (0..candidates.len())
            .filter(|index| !collapse.suppressed.contains(index))
            .collect();
        let mut local_indices = BTreeMap::new();
        for (local, original) in indices.iter().enumerate() {
            local_indices.insert(*original, local);
        }
        let required_spans = conflict_required_spans(candidates, findings)?;
        let mut conflict_units = Vec::new();
        for finding in findings {
            let members = finding
                .candidate_indices
                .iter()
                .map(|original| {
                    local_indices
                        .get(original)
                        .copied()
                        .ok_or_else(|| integrity("conflict member was suppressed as an alternate"))
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
            conflict_units.push(members);
        }
        let selection_candidates = indices
            .iter()
            .map(|original| {
                let candidate = candidates
                    .get(*original)
                    .ok_or_else(|| integrity("selection candidate is missing"))?;
                let source = self
                    .sources
                    .get(&candidate.seeds.revision_id)
                    .ok_or_else(|| integrity("selection source was not cached"))?;
                Ok(SelectionCandidate {
                    markdown: &source.markdown,
                    document: &source.canonical,
                    sections: &source.sections,
                    seeds: candidate.seeds.clone(),
                    required_span: *required_spans
                        .get(*original)
                        .ok_or_else(|| integrity("selection required span is missing"))?,
                    expansion: candidate.expansion.clone(),
                    features: candidate.features.clone(),
                    template: candidate.template.clone(),
                    input_position: candidate.input_position,
                })
            })
            .collect::<Result<Vec<_>, EvidenceError>>()?;
        let budget = SelectionBudget {
            max_passages: usize::try_from(self.input.budget.k)
                .map_err(|_| invalid("passage budget does not fit this target"))?,
            max_tokens: self.input.budget.max_tokens,
            counter: self.counter,
            counter_info,
            control: self.control,
        };
        selection::select(&selection_candidates, &conflict_units, &budget)
            .map(|result| (result, indices))
    }
}

/// Checks immutable generation identity and the current publication lifecycle.
fn validate_generation(pinned: &Generation, current: &Generation) -> Result<(), EvidenceError> {
    if pinned.id != current.id
        || pinned.collection_id != current.collection_id
        || pinned.chunk_set_id != current.chunk_set_id
        || pinned.embedding_profile != current.embedding_profile
        || pinned.sparse_profile != current.sparse_profile
    {
        return Err(integrity("pinned generation identity changed"));
    }
    if !matches!(
        current.state,
        GenerationState::Published | GenerationState::Retired
    ) {
        return Err(EvidenceError::NotVisible);
    }
    Ok(())
}

/// Reads a literal string field from immutable revision metadata.
fn metadata_string(revision: &Revision, key: &str) -> Option<String> {
    revision
        .metadata
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

/// Converts a source integrity failure without including private identifiers.
pub(super) fn integrity(reason: &str) -> EvidenceError {
    EvidenceError::Integrity(reason.to_owned())
}

/// Converts an invalid handoff field into a non-sensitive public error.
pub(super) fn invalid(reason: &str) -> EvidenceError {
    EvidenceError::InvalidRequest(reason.to_owned())
}

/// Checks cancellation and the original absolute deadline.
pub(super) fn check(control: &ReadControl) -> Result<(), EvidenceError> {
    if control.cancelled.load(Ordering::Relaxed) || Instant::now() >= control.deadline {
        Err(EvidenceError::TimedOut)
    } else {
        Ok(())
    }
}

/// Maps retrieval interruption and inaccessible records at the API boundary.
fn map_kernel_error(error: RetrievalError) -> EvidenceError {
    match error {
        RetrievalError::Cancelled | RetrievalError::TimedOut => EvidenceError::TimedOut,
        RetrievalError::UnknownOrInaccessible => EvidenceError::NotVisible,
        other => EvidenceError::Kernel(other),
    }
}
