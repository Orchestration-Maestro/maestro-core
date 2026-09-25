//! Structural preparation and packing; only the public native path certifies counts.
use crate::{
    Block, CanonicalDocument, Error,
    chunks::{ChunkContent, Fragment, InputRole, MappedDocument, TableWindow},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

mod context;
mod layout;
mod prepare;
#[cfg(test)]
mod tests;

/// The size a draft grows toward: once combined drafts reach this many tokens, the draft is
/// complete.
pub(crate) const TARGET_TOKENS: usize = 500;
/// The hard limit on a prepared input's tokens; larger content splits and is never clipped.
pub(crate) const MAX_TOKENS: usize = 700;
/// Characters after which whitespace ends a sentence: ASCII and full-width.
const SENTENCE_ENDS: [char; 6] = ['.', '!', '?', '。', '！', '？'];

/// The refusal when a chunk's context or structure cannot be represented within the measured
/// profile.
fn structure_error() -> Error {
    Error("chunk context/structure cannot be represented under the measured profile".into())
}

/// The source a chunk carries: its fragments and, inside a table, their row windows.
#[derive(Clone, PartialEq)]
struct Body {
    /// The primary-text fragments, in source order.
    fragments: Vec<Fragment>,
    /// The table rows and columns the fragments fall in; empty outside tables.
    windows: Vec<TableWindow>,
}

/// A document's structure indexed for chunking: its blocks by identifier and each unit's chain of
/// ancestor blocks.
struct Layout<'a> {
    /// The original Markdown the units map into.
    markdown: &'a str,
    /// The document's mapped units.
    mapped: &'a MappedDocument,
    /// The document's blocks by identifier.
    blocks: BTreeMap<&'a str, &'a Block>,
    /// For each unit, its blocks from the outermost to the one that owns it.
    ancestry: Vec<Vec<&'a Block>>,
}

/// Context a chunk repeats before its body: which units, in which role and at which depth.
struct ContextEntry {
    /// The input role the repeated units take.
    role: InputRole,
    /// Nesting depth, which orders entries of the same role.
    depth: usize,
    /// The units repeated, in order.
    units: Vec<usize>,
    /// For a table header, the units of each column of the window.
    columns: Option<Vec<Vec<usize>>>,
}

/// Index a document's blocks and each unit's ancestry; a missing or cyclic parent is a structure
/// error.
fn layout<'a>(
    document: &'a CanonicalDocument,
    markdown: &'a str,
    mapped: &'a MappedDocument,
) -> Result<Layout<'a>, Error> {
    let blocks: BTreeMap<_, _> = document
        .blocks
        .iter()
        .map(|block| (block.block_id.as_str(), block))
        .collect();
    let mut ancestry = Vec::new();
    for unit in &mapped.units {
        let mut path = Vec::new();
        let mut current = Some(unit.block_id.as_str());
        while let Some(id) = current {
            let block = blocks.get(id).copied().ok_or_else(structure_error)?;
            if path.iter().any(|ancestor: &&Block| ancestor.block_id == id) {
                return Err(structure_error());
            }
            path.push(block);
            current = block.parent_block_id.as_deref();
        }
        path.reverse();
        ancestry.push(path);
    }
    Ok(Layout {
        markdown,
        mapped,
        blocks,
        ancestry,
    })
}

/// Replay each chunk's preparation: its table windows must match its fragments, and preparing its
/// body again must give the same chunk.
pub(crate) fn validate_preparation(
    document: &CanonicalDocument,
    markdown: &str,
    mapped: &MappedDocument,
    chunks: &[ChunkContent],
    count: &mut impl FnMut(&str) -> Result<usize, Error>,
) -> Result<(), Error> {
    let layout = layout(document, markdown, mapped)?;
    for chunk in chunks {
        let mut witnessed = BTreeSet::new();
        for fragment in &chunk.fragments {
            let index = fragment.contribution.unit_index;
            if index >= mapped.units.len() {
                return Err(structure_error());
            }
            if let Some(expected) = layout.window(index, false)? {
                let matching: Vec<_> = chunk
                    .table_windows
                    .iter()
                    .enumerate()
                    .filter(|(_, window)| {
                        window.table_id == expected.table_id
                            && window.row_id == expected.row_id
                            && window.row_index == expected.row_index
                            && window.columns.contains(&expected.columns[0])
                    })
                    .collect();
                if matching.len() != 1 {
                    return Err(structure_error());
                }
                witnessed.insert(matching[0].0);
            } else if !chunk.table_windows.is_empty() {
                return Err(structure_error());
            }
        }
        if witnessed.len() != chunk.table_windows.len() {
            return Err(structure_error());
        }
        for window in &chunk.table_windows {
            let width = layout.row_cells(&window.row_id)?.len();
            if window.columns.is_empty()
                || window.columns.iter().any(|&column| column >= width)
                || window.columns.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(structure_error());
            }
        }
        if chunk.table_windows.windows(2).any(|pair| {
            pair[0].table_id != pair[1].table_id
                || pair[0].row_index >= pair[1].row_index
                || pair[0].columns != pair[1].columns
        }) {
            return Err(structure_error());
        }
        let body = Body {
            fragments: chunk.fragments.clone(),
            windows: chunk.table_windows.clone(),
        };
        if layout.prepare(&body, count)? != *chunk {
            return Err(structure_error());
        }
    }
    Ok(())
}

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
            if layout.compatible(&body, &atom) {
                let combined = combine(&body, &atom);
                let candidate = layout.prepare(&combined, count)?;
                if candidate.token_count <= MAX_TOKENS {
                    if candidate.token_count >= TARGET_TOKENS {
                        result.push(candidate);
                    } else {
                        current = Some((combined, candidate));
                    }
                    continue;
                }
            }
            result.push(prepared);
        }
        let prepared = layout.prepare(&atom, count)?;
        if prepared.token_count <= MAX_TOKENS {
            if prepared.token_count >= TARGET_TOKENS {
                result.push(prepared);
            } else {
                current = Some((atom, prepared));
            }
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
