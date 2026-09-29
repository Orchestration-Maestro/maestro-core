//! Whole matched-block windows and contiguous table-header prefixes.

use super::super::selection_candidate::SelectionCandidate;
use super::super::{
    sections::{block_span, contains},
    types::EvidenceError,
};
use maestro_canonicalization::BlockType;
use maestro_kernel::evidence::Span;
use std::collections::BTreeMap;

/// Keeps a table's header through its last required whole row. Other blocks
/// retain their canonical sibling envelope, including complete list procedures.
pub(super) fn relevant_span(candidate: &SelectionCandidate<'_>) -> Result<Span, EvidenceError> {
    let seed = candidate.required_span;
    let mandatory = candidate
        .expansion
        .window_plan(seed)
        .map_err(invalid)?
        .mandatory;
    for table in candidate
        .document
        .blocks
        .iter()
        .filter(|block| block.block_type == BlockType::Table)
    {
        let Some(table_span) = block_span(table, candidate.markdown).map_err(invalid)? else {
            continue;
        };
        // Nested tables retain their enclosing list or other prerequisite block.
        if table_span != mandatory || !contains(table_span, seed) {
            continue;
        }
        for row in candidate.document.blocks.iter().filter(|block| {
            block.parent_block_id.as_deref() == Some(table.block_id.as_str())
                && block.block_type == BlockType::TableRow
        }) {
            let Some(row_span) = block_span(row, candidate.markdown).map_err(invalid)? else {
                continue;
            };
            if row_span.start < seed.end && seed.end <= row_span.end {
                return Ok(Span {
                    start: table_span.start,
                    end: row_span.end,
                });
            }
        }
    }
    Ok(mandatory)
}

/// Retains the stable integrity boundary for canonical source inconsistencies.
fn invalid(_: String) -> EvidenceError {
    EvidenceError::Integrity("candidate has no safe relevant source window".to_owned())
}

/// Counts the table-prefix windows among `spans`, for the omission gap.
pub(super) fn table_prefixes(
    candidates: &[SelectionCandidate<'_>],
    spans: &BTreeMap<usize, Span>,
) -> usize {
    spans
        .iter()
        .filter(|(index, span)| {
            candidates.get(**index).is_some_and(|candidate| {
                candidate
                    .expansion
                    .window_plan(candidate.required_span)
                    .is_ok_and(|plan| plan.mandatory != **span)
            })
        })
        .count()
}
