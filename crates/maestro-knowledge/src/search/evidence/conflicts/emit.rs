//! Converts selected conflict facts into passage-numbered bundle signals.

use super::super::signals::WithinPassageConflict;
use super::detect::{ConflictFinding, ConflictSource};
use maestro_kernel::evidence::{Conflict, Passage, Span};
use std::collections::{BTreeMap, BTreeSet};

/// Conflicts split across passages and disagreements retained within one passage.
pub(crate) struct ConflictEmission<'a> {
    /// Distinct conflict keys with at least two source passages.
    pub(crate) conflicts: Vec<Conflict>,
    /// Distinct values that coalesced into the same passage.
    pub(crate) within_passage: Vec<WithinPassageConflict<'a>>,
}

/// Emits only complete conflicts whose every candidate survived selection.
pub(crate) fn emit_conflicts<'a>(
    findings: &'a [ConflictFinding],
    sources: &[ConflictSource<'_>],
    selected_candidates: &BTreeSet<usize>,
    passages: &[Passage],
) -> Result<ConflictEmission<'a>, String> {
    let sources = source_index(sources)?;
    let mut conflicts = Vec::new();
    let mut within_passage = Vec::new();
    for finding in findings {
        if finding.values.len() < 2 {
            return Err("conflict finding has fewer than two explicit values".to_owned());
        }
        let selected_count = finding
            .candidate_indices
            .intersection(selected_candidates)
            .count();
        if selected_count == 0 {
            continue;
        }
        if selected_count != finding.candidate_indices.len() {
            return Err("selection returned only one side of a conflict".to_owned());
        }

        let mut passage_values = BTreeMap::<u32, BTreeSet<String>>::new();
        for candidate_index in &finding.candidate_indices {
            add_candidate_values(
                finding,
                &sources,
                passages,
                *candidate_index,
                &mut passage_values,
            )?;
        }
        if passage_values.is_empty() {
            return Err("selected conflict has no retained table spans".to_owned());
        }
        for (number, values) in &passage_values {
            if values.len() > 1 {
                within_passage.push(WithinPassageConflict {
                    passage_number: *number,
                    entity: &finding.entity,
                    attribute: &finding.attribute,
                });
            }
        }
        if passage_values.len() >= 2 {
            conflicts.push(Conflict {
                entity: finding.entity.clone(),
                attribute: finding.attribute.clone(),
                passages: passage_values.keys().copied().collect(),
            });
        }
    }
    conflicts.sort_by(|left, right| {
        left.entity
            .cmp(&right.entity)
            .then_with(|| left.attribute.cmp(&right.attribute))
            .then_with(|| left.passages.cmp(&right.passages))
    });
    conflicts.dedup();
    within_passage
        .sort_by_key(|finding| (finding.passage_number, finding.entity, finding.attribute));
    within_passage.dedup_by(|left, right| {
        left.passage_number == right.passage_number
            && left.entity == right.entity
            && left.attribute == right.attribute
    });
    Ok(ConflictEmission {
        conflicts,
        within_passage,
    })
}

/// Adds one candidate's validated table values to their containing passages.
fn add_candidate_values(
    finding: &ConflictFinding,
    sources: &BTreeMap<usize, &ConflictSource<'_>>,
    passages: &[Passage],
    candidate_index: usize,
    passage_values: &mut BTreeMap<u32, BTreeSet<String>>,
) -> Result<(), String> {
    let source = sources
        .get(&candidate_index)
        .ok_or_else(|| "conflict candidate source is missing".to_owned())?;
    let table_values = finding
        .values_by_table_span
        .get(&candidate_index)
        .ok_or_else(|| "conflict candidate table values are missing".to_owned())?;
    for (table_span, values) in table_values {
        if values.is_empty() {
            return Err("conflict table has no explicit values".to_owned());
        }
        let passage = passage_for_table(passages, source.revision_id, *table_span)?;
        passage_values
            .entry(passage.n)
            .or_default()
            .extend(values.iter().cloned());
    }
    Ok(())
}

/// Returns the unique numbered passage containing one source table span.
fn passage_for_table<'a>(
    passages: &'a [Passage],
    revision_id: &str,
    table_span: Span,
) -> Result<&'a Passage, String> {
    let mut containing = passages.iter().filter(|passage| {
        passage.revision_id == revision_id
            && passage.span.start <= table_span.start
            && passage.span.end >= table_span.end
    });
    let passage = containing
        .next()
        .ok_or_else(|| "selected conflict table is absent from its passage".to_owned())?;
    if containing.next().is_some() || passage.n == 0 {
        return Err("conflict table has ambiguous passage provenance".to_owned());
    }
    Ok(passage)
}

/// Indexes every candidate exactly once for later final-source validation.
fn source_index<'source, 'data>(
    sources: &'source [ConflictSource<'data>],
) -> Result<BTreeMap<usize, &'source ConflictSource<'data>>, String> {
    let mut indexed = BTreeMap::new();
    for source in sources {
        if indexed.insert(source.candidate_index, source).is_some() {
            return Err("conflict candidate source is repeated".to_owned());
        }
    }
    Ok(indexed)
}
