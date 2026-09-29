//! Canonical source ancestry and stable identifiers for graph groups.
use super::types::{DeliveryUnit, SourcePart, UnitGraphInput};
use crate::{
    content::{Block, BlockType},
    error::Error,
    hashing::digest,
    source_units::MappedDocument,
};
use serde::Serialize;

/// Build the sorted source contributions for one row group.
pub(super) fn row_parts(
    row_units: &[&DeliveryUnit],
    row: &Block,
    table_parts: &[SourcePart],
    input: &UnitGraphInput<'_>,
    mapped: &MappedDocument,
) -> Vec<SourcePart> {
    if row_units.is_empty() {
        table_parts
            .iter()
            .filter(|part| part_maps_under(part, row, input, mapped))
            .cloned()
            .collect()
    } else {
        row_units
            .iter()
            .flat_map(|unit| unit.parts.iter().cloned())
            .collect()
    }
}

/// Whether a delivery unit maps to a block below the given ancestor.
pub(super) fn unit_maps_under(
    unit: &DeliveryUnit,
    ancestor: &Block,
    input: &UnitGraphInput<'_>,
    mapped: &MappedDocument,
) -> bool {
    unit.parts
        .iter()
        .any(|part| part_maps_under(part, ancestor, input, mapped))
}

/// Whether a part maps to a descendant of a canonical block.
pub(super) fn part_maps_under(
    part: &SourcePart,
    ancestor: &Block,
    input: &UnitGraphInput<'_>,
    mapped: &MappedDocument,
) -> bool {
    part.mappings.iter().any(|mapping| {
        let Some(source) = mapped
            .units
            .iter()
            .find(|source| source.unit_id == mapping.unit_id)
        else {
            return false;
        };
        let Some(mut block) = input
            .document
            .blocks
            .iter()
            .find(|block| block.block_id == source.block_id)
        else {
            return false;
        };
        loop {
            if block.block_id == ancestor.block_id {
                return true;
            }
            let Some(parent) = block.parent_block_id.as_deref() else {
                return false;
            };
            let Some(next) = input
                .document
                .blocks
                .iter()
                .find(|candidate| candidate.block_id == parent)
            else {
                return false;
            };
            block = next;
        }
    })
}

/// Check whether the block is equal to or below the ancestor in canonical parent links.
pub(super) fn block_below(input: &UnitGraphInput<'_>, block: &Block, ancestor: &str) -> bool {
    let mut current = block;
    loop {
        if current.block_id == ancestor {
            return true;
        }
        let Some(parent) = current.parent_block_id.as_deref() else {
            return false;
        };
        let Some(next) = input
            .document
            .blocks
            .iter()
            .find(|candidate| candidate.block_id == parent)
        else {
            return false;
        };
        current = next;
    }
}

/// The nearest preceding paragraph in the same section, used as a real lead-in.
pub(super) fn preceding_lead_in<'a>(
    input: &'a UnitGraphInput<'_>,
    block: &Block,
) -> Option<&'a Block> {
    let index = input
        .document
        .blocks
        .iter()
        .position(|candidate| candidate.block_id == block.block_id)?;
    input
        .document
        .blocks
        .iter()
        .take(index)
        .rev()
        .find(|candidate| {
            candidate.block_type == BlockType::Paragraph
                && candidate.parent_section_id == block.parent_section_id
        })
}

/// Stable group identity for one canonical section.
pub(super) fn section_group_id(
    input: &UnitGraphInput<'_>,
    section_id: &str,
) -> Result<String, Error> {
    Ok(format!(
        "section-{}",
        sha(&(
            &input.document.document_id,
            &input.document.revision_id,
            section_id
        ))?
    ))
}

/// Stable identity for a table, procedure or code group.
pub(super) fn structural_group_id(
    input: &UnitGraphInput<'_>,
    block: &Block,
) -> Result<String, Error> {
    Ok(format!(
        "group-{}",
        sha(&(
            &input.document.document_id,
            &input.document.revision_id,
            &block.block_id
        ))?
    ))
}

/// Apply the pinned whitespace-fold and lowercase family normalization.
pub(super) fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Hash deterministic JSON identity bytes.
pub(super) fn sha(value: &impl Serialize) -> Result<String, Error> {
    let bytes = serde_json::to_vec(value).map_err(|error| Error(error.to_string()))?;
    Ok(digest(&bytes))
}
