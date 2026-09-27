//! Builds source candidates and derives their version/conflict membership.

use super::super::super::request::EvidenceInput;
use super::{
    super::{
        conflicts::detect::{ConflictFinding, ConflictSource},
        families::{CandidateFamily, ConflictContext},
        features::diversity_features,
        source::SourceCache,
        spans::SpanUnion,
        types::EvidenceError,
        versions::{VersionCandidate, VersionCollapse, collapse_versions},
    },
    types::CandidateData,
};
use crate::query::QueryKind;
use maestro_kernel::{
    artifact::Digest,
    document::Revision,
    evidence::{Alternate, Inventory, Passage, Span},
};
use std::collections::{BTreeMap, BTreeSet};

/// Joins one selected candidate pool to its cached canonical source records.
pub(super) fn make_candidate_data(
    seeds: SpanUnion,
    sources: &SourceCache<'_>,
    near_groups: &BTreeMap<String, BTreeSet<String>>,
) -> Result<CandidateData, EvidenceError> {
    let source = sources
        .get(&seeds.revision_id)
        .ok_or_else(|| integrity("candidate source was not cached"))?;
    let expansion = source
        .sections
        .expand(&seeds)
        .map_err(|_| integrity("candidate has no valid canonical expansion"))?;
    let text = source
        .markdown
        .get(seeds.span.start..seeds.span.end)
        .ok_or_else(|| integrity("candidate seed is not a UTF-8 source span"))?;
    let version = metadata_string(&source.revision, "version");
    let context = ConflictContext {
        product: metadata_string(&source.revision, "product"),
        component: metadata_string(&source.revision, "component"),
        platform: metadata_string(&source.revision, "platform"),
        lang: metadata_string(&source.revision, "lang"),
    };
    let occurrence = match expansion.section_id.as_deref() {
        Some(section_id) => {
            let occurrence = source
                .section_occurrences
                .get(section_id)
                .ok_or_else(|| integrity("candidate section occurrence is missing"))?;
            if occurrence.section_path != expansion.section_path {
                return Err(integrity("candidate section path is inconsistent"));
            }
            occurrence.occurrence
        }
        None => 1,
    };
    let family = CandidateFamily {
        document_id: source.document.id.clone(),
        near_group_ids: near_groups
            .get(&seeds.revision_id)
            .cloned()
            .unwrap_or_default(),
        section_path: expansion.section_path.clone(),
        occurrence,
        context,
    };
    let features = diversity_features(
        &source.markdown,
        &source.canonical,
        expansion.extent,
        version.clone(),
    )
    .map_err(|_| integrity("candidate diversity features are invalid"))?;
    let template = Passage {
        n: 0,
        section_id: expansion.section_id.clone(),
        document_id: source.document.id.clone(),
        revision_id: source.revision.id.clone(),
        title: source
            .canonical
            .source_metadata
            .title
            .clone()
            .unwrap_or_default(),
        section_path: expansion.section_path.clone(),
        version,
        source_ref: source.document.source_ref.clone(),
        span: seeds.span,
        digest: Digest::of(text.as_bytes()),
        text: text.to_owned(),
        windowed: false,
        alternates: Vec::<Alternate>::new(),
    };
    let input_position = seeds
        .seeds
        .iter()
        .map(|seed| seed.input_position)
        .min()
        .ok_or_else(|| integrity("candidate union has no source seeds"))?;
    Ok(CandidateData {
        seeds,
        expansion,
        family,
        template,
        features,
        input_position,
    })
}

/// Builds conflict-family views without exposing source text in diagnostics.
pub(super) fn conflict_sources<'a>(
    candidates: &'a [CandidateData],
    sources: &'a SourceCache<'_>,
) -> Result<Vec<ConflictSource<'a>>, EvidenceError> {
    candidates
        .iter()
        .enumerate()
        .map(|(candidate_index, candidate)| {
            let source = sources
                .get(&candidate.seeds.revision_id)
                .ok_or_else(|| integrity("conflict source was not cached"))?;
            Ok(ConflictSource {
                candidate_index,
                document_id: &source.document.id,
                revision_id: &source.revision.id,
                near_group_ids: &candidate.family.near_group_ids,
                context: &candidate.family.context,
                proposed_extent: candidate.expansion.extent,
                document: &source.canonical,
                markdown: &source.markdown,
            })
        })
        .collect()
}

/// Returns full proposed passage text from the authorized original source.
pub(super) fn candidate_texts(
    candidates: &[CandidateData],
    sources: &SourceCache<'_>,
) -> Result<Vec<String>, EvidenceError> {
    candidates
        .iter()
        .map(|candidate| {
            let source = sources
                .get(&candidate.seeds.revision_id)
                .ok_or_else(|| integrity("candidate source was not cached"))?;
            source
                .markdown
                .get(candidate.expansion.extent.start..candidate.expansion.extent.end)
                .map(str::to_owned)
                .ok_or_else(|| integrity("candidate expansion is not a UTF-8 source span"))
        })
        .collect()
}

/// Builds exact-section version candidates and applies only approved collapse modes.
pub(super) fn collapse_candidate_versions(
    input: &EvidenceInput,
    candidates: &mut [CandidateData],
    findings: &[ConflictFinding],
    sources: &SourceCache<'_>,
) -> Result<VersionCollapse, EvidenceError> {
    let conflict_members: BTreeSet<_> = findings
        .iter()
        .flat_map(|finding| finding.candidate_indices.iter().copied())
        .collect();
    let version_candidates = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let source = sources
                .get(&candidate.seeds.revision_id)
                .ok_or_else(|| integrity("version source was not cached"))?;
            let section_text = source
                .markdown
                .get(candidate.expansion.extent.start..candidate.expansion.extent.end)
                .ok_or_else(|| integrity("version section is not a UTF-8 source span"))?;
            Ok(VersionCandidate {
                family: candidate.family.clone(),
                input_position: candidate.input_position,
                revision_id: candidate.seeds.revision_id.clone(),
                section_id: candidate.expansion.section_id.clone(),
                version: candidate.template.version.clone(),
                section_text: section_text.to_owned(),
                conflict_member: conflict_members.contains(&index),
            })
        })
        .collect::<Result<Vec<_>, EvidenceError>>()?;
    let enabled = input.version.is_none()
        && input.understood.kind != QueryKind::Comparison
        && !matches!(input.inventory.as_ref(), Some(Inventory::Versions { .. }));
    let collapse = collapse_versions(&version_candidates, enabled)
        .map_err(|_| integrity("version correspondence data is invalid"))?;
    for (index, alternates) in &collapse.alternates {
        candidates
            .get_mut(*index)
            .ok_or_else(|| integrity("version representative index is invalid"))?
            .template
            .alternates
            .clone_from(alternates);
    }
    Ok(collapse)
}

/// Extends conflict members' mandatory spans through every complete fact table.
pub(super) fn conflict_required_spans(
    candidates: &[CandidateData],
    findings: &[ConflictFinding],
) -> Result<Vec<Span>, EvidenceError> {
    let mut required: Vec<_> = candidates
        .iter()
        .map(|candidate| candidate.seeds.span)
        .collect();
    for finding in findings {
        for (index, table_spans) in &finding.table_spans {
            let candidate = candidates
                .get(*index)
                .ok_or_else(|| integrity("conflict table names a missing candidate"))?;
            let span = required
                .get_mut(*index)
                .ok_or_else(|| integrity("conflict required span is missing"))?;
            include_conflict_table_spans(span, candidate.expansion.extent, table_spans)?;
        }
    }
    Ok(required)
}

/// Maps selected local candidate indexes back to the original conflict pool.
pub(super) fn selected_originals(
    selected: &BTreeSet<usize>,
    indices: &[usize],
) -> Result<BTreeSet<usize>, EvidenceError> {
    selected
        .iter()
        .map(|local| {
            indices
                .get(*local)
                .copied()
                .ok_or_else(|| integrity("selected candidate index is invalid"))
        })
        .collect()
}

/// Reads a literal string field from immutable revision metadata.
fn metadata_string(revision: &Revision, key: &str) -> Option<String> {
    revision
        .metadata
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

/// Extends a mandatory seed through validated complete conflict-table spans.
fn include_conflict_table_spans(
    required: &mut Span,
    extent: Span,
    tables: &[Span],
) -> Result<(), EvidenceError> {
    for table in tables {
        if table.start >= table.end || table.start < extent.start || table.end > extent.end {
            return Err(integrity("conflict table is outside its candidate extent"));
        }
        required.start = required.start.min(table.start);
        required.end = required.end.max(table.end);
    }
    Ok(())
}

/// Converts a source integrity failure without including private identifiers.
fn integrity(reason: &str) -> EvidenceError {
    EvidenceError::Integrity(reason.to_owned())
}
