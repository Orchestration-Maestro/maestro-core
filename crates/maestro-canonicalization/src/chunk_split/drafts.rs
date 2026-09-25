//! Packing a document's atoms into drafts: combined up to the target, refined or split past the
//! maximum.
use super::limits::{MAX_TOKENS, TARGET_TOKENS};
use super::refusal::structure_error;
use super::structure::{Body, Layout, layout};
use crate::{
    document::CanonicalDocument, error::Error, prepared_inputs::ChunkContent,
    source_units::MappedDocument,
};
use std::collections::{BTreeMap, VecDeque};

/// Pack a document's atoms into drafts: combine compatible atoms up to the target, refine or split
/// what exceeds the maximum, then number each unit's parts. A refinement that hands a body back
/// unchanged would never end, so it is a structure error.
pub(crate) fn build_drafts(
    document: &CanonicalDocument,
    markdown: &str,
    mapped: &MappedDocument,
    count: &mut impl FnMut(&str) -> Result<usize, Error>,
) -> Result<Vec<ChunkContent>, Error> {
    let layout = layout(document, markdown, mapped)?;
    let mut pending: VecDeque<_> = layout.atoms()?.into();
    let mut current: Option<(Body, ChunkContent)> = None;
    let mut result = Vec::new();
    while let Some(atom) = pending.pop_front() {
        if let Some((body, prepared)) = current.take() {
            if let Some((combined, candidate)) = grown(&layout, &body, &atom, count)? {
                current = settle(&mut result, combined, candidate);
                continue;
            }
            result.push(prepared);
        }
        let prepared = layout.prepare(&atom, count)?;
        if prepared.token_count <= MAX_TOKENS {
            current = settle(&mut result, atom, prepared);
        } else if let Some(refined) = layout.refine(&atom)? {
            if refined.contains(&atom) {
                return Err(structure_error());
            }
            for piece in refined.into_iter().rev() {
                pending.push_front(piece);
            }
        } else {
            result.extend(layout.split_unit(&atom, count)?);
        }
    }
    if let Some((_, prepared)) = current {
        result.push(prepared);
    }
    let mut ordinals = BTreeMap::new();
    for chunk in &mut result {
        for fragment in &mut chunk.fragments {
            let ordinal = ordinals
                .entry(fragment.contribution.unit_index)
                .or_insert(0);
            fragment.part_ordinal = *ordinal;
            *ordinal += 1;
        }
    }
    Ok(result)
}

/// The open draft grown by an atom, when the two are compatible and their combined chunk stays
/// within the maximum.
fn grown(
    layout: &Layout<'_>,
    body: &Body,
    atom: &Body,
    count: &mut impl FnMut(&str) -> Result<usize, Error>,
) -> Result<Option<(Body, ChunkContent)>, Error> {
    if !layout.compatible(body, atom) {
        return Ok(None);
    }
    let combined = combine(body, atom);
    let candidate = layout.prepare(&combined, count)?;
    Ok((candidate.token_count <= MAX_TOKENS).then_some((combined, candidate)))
}

/// A draft that reached the target is complete and joins the result; a smaller one stays open.
fn settle(
    result: &mut Vec<ChunkContent>,
    body: Body,
    prepared: ChunkContent,
) -> Option<(Body, ChunkContent)> {
    if prepared.token_count >= TARGET_TOKENS {
        result.push(prepared);
        None
    } else {
        Some((body, prepared))
    }
}

/// Two bodies as one: the fragments appended, and the column windows of one row merged.
fn combine(left: &Body, right: &Body) -> Body {
    let mut result = left.clone();
    result.fragments.extend(right.fragments.iter().cloned());
    for window in &right.windows {
        if let Some(last) = result
            .windows
            .last_mut()
            .filter(|last| last.row_id == window.row_id)
        {
            last.columns.extend(&window.columns);
            last.columns.sort_unstable();
            last.columns.dedup();
        } else {
            result.windows.push(window.clone());
        }
    }
    result
}
