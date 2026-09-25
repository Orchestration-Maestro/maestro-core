//! Replaying a chunk's preparation: its table windows against its fragments, then the chunk
//! prepared again from its body.
use super::refusal::structure_error;
use super::structure::{Body, Layout, layout};
use crate::{
    CanonicalDocument, Error,
    chunks::{ChunkContent, MappedDocument, TableWindow},
};
use std::collections::BTreeSet;

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
        witness_windows(&layout, chunk)?;
        order_windows(&layout, chunk)?;
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

/// Each fragment inside a table falls in exactly one of the chunk's windows and every window holds
/// one; a chunk outside tables has no window.
fn witness_windows(layout: &Layout<'_>, chunk: &ChunkContent) -> Result<(), Error> {
    let mut witnessed = BTreeSet::new();
    for fragment in &chunk.fragments {
        let index = fragment.contribution.unit_index;
        if index >= layout.mapped.units.len() {
            return Err(structure_error());
        }
        if let Some(expected) = layout.window(index, false)? {
            witnessed.insert(matching_window(chunk, &expected)?);
        } else if !chunk.table_windows.is_empty() {
            return Err(structure_error());
        }
    }
    if witnessed.len() != chunk.table_windows.len() {
        return Err(structure_error());
    }
    Ok(())
}

/// The position of the one window of a chunk that holds a fragment's own table cell.
fn matching_window(chunk: &ChunkContent, expected: &TableWindow) -> Result<usize, Error> {
    let column = expected.columns.first().ok_or_else(structure_error)?;
    let matching: Vec<_> = chunk
        .table_windows
        .iter()
        .enumerate()
        .filter(|(_, window)| {
            window.table_id == expected.table_id
                && window.row_id == expected.row_id
                && window.row_index == expected.row_index
                && window.columns.contains(column)
        })
        .map(|(position, _)| position)
        .collect();
    match matching.as_slice() {
        [only] => Ok(*only),
        _ => Err(structure_error()),
    }
}

/// Each window names columns of its row in increasing order, and the windows follow the rows of
/// one table in order over the same columns.
fn order_windows(layout: &Layout<'_>, chunk: &ChunkContent) -> Result<(), Error> {
    for window in &chunk.table_windows {
        let width = layout.row_cells(&window.row_id)?.len();
        if window.columns.is_empty()
            || window.columns.iter().any(|&column| column >= width)
            || window
                .columns
                .iter()
                .zip(window.columns.iter().skip(1))
                .any(|(left, right)| left >= right)
        {
            return Err(structure_error());
        }
    }
    let windows = &chunk.table_windows;
    if windows
        .iter()
        .zip(windows.iter().skip(1))
        .any(|(left, right)| {
            left.table_id != right.table_id
                || left.row_index >= right.row_index
                || left.columns != right.columns
        })
    {
        return Err(structure_error());
    }
    Ok(())
}
