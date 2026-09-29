//! Whole-table packing when its complete prepared input fits the pinned counter limit.
use super::{
    prepared::{prepared_text, size_limit},
    profile::UnitProfile,
    types::{DeliveryUnit, PartRole, SplitMarker, UnitGraphInput, UnitKind},
};
use crate::{
    content::{Block, BlockType},
    error::Error,
    hashing::digest,
    source_units::MappedDocument,
    tokenizer::TokenCounter,
};
use std::collections::BTreeSet;

/// Replace complete source rows with one complete-table unit only when the verified counter fits.
pub(super) fn pack_small_tables(
    input: &UnitGraphInput<'_>,
    mapped: &MappedDocument,
    units: &mut Vec<DeliveryUnit>,
    profile: UnitProfile,
    counter: &(impl TokenCounter + ?Sized),
) -> Result<(), Error> {
    let tables: Vec<_> = input
        .document
        .blocks
        .iter()
        .filter(|block| block.block_type == BlockType::Table)
        .collect();
    for table in tables {
        let source_blocks: BTreeSet<_> = input
            .document
            .blocks
            .iter()
            .filter(|block| {
                matches!(block.block_type, BlockType::TableHead | BlockType::TableRow)
                    && block_below(input, block, &table.block_id)
            })
            .map(|block| block.block_id.as_str())
            .collect();
        let candidates: Vec<_> = units
            .iter()
            .filter(|unit| source_blocks.contains(unit.source_block_id.as_str()))
            .collect();
        if candidates.is_empty() {
            continue;
        }
        let candidate_ids: BTreeSet<_> =
            candidates.iter().map(|unit| unit.unit_id.clone()).collect();
        let complete = complete_table_unit(table, &candidates, input)?;
        if counter
            .token_ids(&prepared_text(&complete, mapped, &[], &[])?)?
            .len()
            <= size_limit(profile, UnitKind::Table)
        {
            units.retain(|unit| !candidate_ids.contains(&unit.unit_id));
            units.push(complete);
            units.sort_by_key(|unit| {
                (
                    unit.parts
                        .iter()
                        .flat_map(|part| &part.ranges)
                        .map(|range| range.start)
                        .min()
                        .unwrap_or(usize::MAX),
                    unit.unit_id.clone(),
                )
            });
        }
    }
    Ok(())
}

/// Assemble one table's complete primary contribution with a stable identity.
fn complete_table_unit(
    table: &Block,
    candidates: &[&DeliveryUnit],
    input: &UnitGraphInput<'_>,
) -> Result<DeliveryUnit, Error> {
    let mut parts: Vec<_> = candidates
        .iter()
        .flat_map(|unit| &unit.parts)
        .filter(|part| part.role == PartRole::Primary)
        .cloned()
        .collect();
    parts.sort_by_key(|part| {
        part.ranges
            .iter()
            .map(|range| range.start)
            .min()
            .unwrap_or(usize::MAX)
    });
    for (ordinal, part) in parts.iter_mut().enumerate() {
        part.ordinal = ordinal;
    }
    let mut identity_units: Vec<_> = candidates
        .iter()
        .map(|unit| unit.unit_id.as_str())
        .collect();
    identity_units.sort_unstable();
    let identity = serde_json::to_vec(&(
        "complete-table-unit/1",
        &input.document.document_id,
        &input.document.revision_id,
        &table.block_id,
        &identity_units,
    ))
    .map_err(|error| Error(error.to_string()))?;
    Ok(DeliveryUnit {
        unit_id: format!("unit-{}", digest(&identity)),
        kind: UnitKind::Table,
        source_block_id: table.block_id.clone(),
        section_id: table.parent_section_id.clone(),
        heading_path: table.heading_path.clone(),
        occurrence: 0,
        parent_id: table
            .parent_block_id
            .as_ref()
            .map(|parent| format!("block-{parent}")),
        parts,
        split: SplitMarker::Whole,
    })
}

/// Check whether the block is equal to or below the ancestor in canonical parent links.
fn block_below(input: &UnitGraphInput<'_>, block: &Block, ancestor: &str) -> bool {
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
