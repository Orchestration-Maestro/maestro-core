//! A document's structure indexed for chunking, the bodies packed from it and the context they
//! repeat.
use super::refusal::structure_error;
use crate::{
    content::Block,
    document::CanonicalDocument,
    error::Error,
    prepared_inputs::{Fragment, InputRole, TableWindow},
    source_units::{MappedDocument, SourceUnit},
};
use std::collections::BTreeMap;

/// The source a chunk carries: its fragments and, inside a table, their row windows.
#[derive(Clone, PartialEq)]
pub(super) struct Body {
    /// The primary-text fragments, in source order.
    pub(super) fragments: Vec<Fragment>,
    /// The table rows and columns the fragments fall in; empty outside tables.
    pub(super) windows: Vec<TableWindow>,
}

/// A document's structure indexed for chunking: its blocks by identifier and each unit's chain of
/// ancestor blocks.
pub(super) struct Layout<'a> {
    /// The original Markdown the units map into.
    pub(super) markdown: &'a str,
    /// The document's mapped units.
    pub(super) mapped: &'a MappedDocument,
    /// The document's blocks by identifier.
    pub(super) blocks: BTreeMap<&'a str, &'a Block>,
    /// For each unit, its blocks from the outermost to the one that owns it.
    ancestry: Vec<Vec<&'a Block>>,
}

/// Context a chunk repeats before its body: which units, in which role and at which depth.
pub(super) struct ContextEntry {
    /// The input role the repeated units take.
    pub(super) role: InputRole,
    /// Nesting depth, which orders entries of the same role.
    pub(super) depth: usize,
    /// The units repeated, in order.
    pub(super) units: Vec<usize>,
    /// For a table header, the units of each column of the window.
    pub(super) columns: Option<Vec<Vec<usize>>>,
}

/// Index a document's blocks and each unit's ancestry; a missing or cyclic parent is a structure
/// error.
pub(super) fn layout<'a>(
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
            if path.iter().any(|seen: &&Block| seen.block_id == id) {
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

impl Layout<'_> {
    /// A unit's blocks from the outermost to the one that owns it; none for an unknown unit.
    pub(super) fn ancestors(&self, unit: usize) -> &[&Block] {
        self.ancestry.get(unit).map_or(&[], Vec::as_slice)
    }

    /// A unit by index; an unknown unit is a structure error.
    pub(super) fn unit(&self, index: usize) -> Result<&SourceUnit, Error> {
        self.mapped.units.get(index).ok_or_else(structure_error)
    }

    /// Whether a unit is primary text; an unknown unit is not.
    pub(super) fn is_primary(&self, index: usize) -> bool {
        self.mapped
            .units
            .get(index)
            .is_some_and(|unit| unit.primary)
    }

    /// Whether a unit belongs to the block `block_id`; an unknown unit belongs to none.
    pub(super) fn in_block(&self, index: usize, block_id: &str) -> bool {
        self.mapped
            .units
            .get(index)
            .is_some_and(|unit| unit.block_id == block_id)
    }
}
