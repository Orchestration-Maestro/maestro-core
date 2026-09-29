//! A document's structure indexed for chunking, the bodies packed from it and the context they
//! repeat.
use super::chrome::{Indexed, indexed};
use super::limits::MAX_TOKENS;
use super::refusal::structure_error;
use crate::{
    chunk_profile::{ChromeRule, ChunkProfile, ChunkRules},
    content::Block,
    document::CanonicalDocument,
    error::Error,
    prepared_inputs::{Fragment, InputRole, TableWindow},
    source_units::{MappedDocument, SourceUnit, TextRange},
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

impl Body {
    /// The body with another appended: its fragments, and the column windows of one row merged.
    pub(super) fn combined(&self, right: &Self) -> Self {
        let mut result = self.clone();
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
}

/// A document's structure indexed for chunking under one profile: its blocks by identifier, each
/// unit's chain of ancestor blocks and the part of each unit the profile indexes.
pub(crate) struct Layout<'a> {
    /// The canonical document the units map.
    pub(crate) document: &'a CanonicalDocument,
    /// The original Markdown the units map into.
    pub(crate) markdown: &'a str,
    /// The document's mapped units.
    pub(crate) mapped: &'a MappedDocument,
    /// The profile the document is chunked under.
    profile: ChunkProfile,
    /// The rules of that profile.
    rules: ChunkRules,
    /// The document's blocks by identifier.
    pub(super) blocks: BTreeMap<&'a str, &'a Block>,
    /// For each unit, its blocks from the outermost to the one that owns it.
    ancestry: Vec<Vec<&'a Block>>,
    /// For each unit, the part of its text the profile indexes, and the chrome rule that left
    /// the rest out.
    indexed: Indexed,
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

impl<'a> Layout<'a> {
    /// Index a document's blocks, each unit's ancestry and the part of each unit `profile`
    /// indexes; a missing or cyclic parent is a structure error.
    pub(crate) fn new(
        document: &'a CanonicalDocument,
        markdown: &'a str,
        mapped: &'a MappedDocument,
        profile: ChunkProfile,
    ) -> Result<Self, Error> {
        let blocks: BTreeMap<_, _> = document
            .blocks
            .iter()
            .map(|block| (block.block_id.as_str(), block))
            .collect();
        let ancestry = mapped
            .units
            .iter()
            .map(|unit| ancestry(&blocks, unit))
            .collect::<Result<_, _>>()?;
        Ok(Layout {
            document,
            markdown,
            mapped,
            profile,
            rules: profile.rules(),
            blocks,
            ancestry,
            indexed: indexed(document, mapped, profile.rules().chrome.as_ref()),
        })
    }
}

/// A unit's blocks from the outermost to the one that owns it; a missing or cyclic parent is a
/// structure error.
fn ancestry<'a>(
    blocks: &BTreeMap<&'a str, &'a Block>,
    unit: &SourceUnit,
) -> Result<Vec<&'a Block>, Error> {
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
    Ok(path)
}

impl Layout<'_> {
    /// The profile the document is chunked under.
    pub(crate) const fn profile(&self) -> ChunkProfile {
        self.profile
    }

    /// The rules the document is chunked under.
    pub(crate) const fn rules(&self) -> &ChunkRules {
        &self.rules
    }

    /// The part of a unit's text the profile indexes; none for page chrome or an unknown unit.
    pub(crate) fn kept(&self, unit: usize) -> Option<TextRange> {
        self.indexed.kept.get(unit).copied().flatten()
    }

    /// The chrome rule that left some of a unit's text out; none when it is indexed whole.
    pub(crate) fn chrome_rule(&self, unit: usize) -> Option<ChromeRule> {
        self.indexed.rules.get(unit).copied().flatten()
    }

    /// A unit's blocks from the outermost to the one that owns it; none for an unknown unit.
    pub(super) fn ancestors(&self, unit: usize) -> &[&Block] {
        self.ancestry.get(unit).map_or(&[], Vec::as_slice)
    }

    /// A unit by index; an unknown unit is a structure error.
    pub(super) fn unit(&self, index: usize) -> Result<&SourceUnit, Error> {
        self.mapped.units.get(index).ok_or_else(structure_error)
    }

    /// The refusal of a unit that cannot fit the hard maximum with its mandatory context: it
    /// names the unit, its block and the block's span in the original Markdown, never their text.
    /// A unit or block the layout does not hold, or a block without a span, is a structure error.
    pub(super) fn oversized(&self, index: usize) -> Error {
        let named = self.mapped.units.get(index).and_then(|unit| {
            let spans = &self.blocks.get(unit.block_id.as_str())?.source_spans;
            let start = spans.iter().map(|span| span.start).min()?;
            let end = spans.iter().map(|span| span.end).max()?;
            Some(Error(format!(
                "the unit {} of block {} at bytes [{start}, {end}) does not fit in {MAX_TOKENS} \
                 tokens with its context",
                unit.unit_id, unit.block_id
            )))
        });
        named.unwrap_or_else(structure_error)
    }

    /// Whether a unit is primary text; an unknown unit is not.
    pub(super) fn is_primary(&self, index: usize) -> bool {
        self.mapped
            .units
            .get(index)
            .is_some_and(|unit| unit.primary)
    }

    /// Whether a unit is primary text the profile indexes, not page chrome.
    pub(super) fn indexed(&self, index: usize) -> bool {
        self.is_primary(index) && self.kept(index).is_some()
    }

    /// Whether a unit belongs to the block `block_id`; an unknown unit belongs to none.
    pub(super) fn in_block(&self, index: usize, block_id: &str) -> bool {
        self.mapped
            .units
            .get(index)
            .is_some_and(|unit| unit.block_id == block_id)
    }
}
