//! Whole-table packing when its complete prepared input fits the pinned counter limit.
use super::{
    group_helpers::{SourceIndex, first_start},
    prepared::{MappedTextIndex, prepared_text, size_limit},
    profile::UnitProfile,
    types::{DeliveryUnit, PartRole, SplitMarker, UnitGraphInput, UnitKind},
};
use crate::{
    content::{Block, BlockType},
    error::Error,
    hashing::digest,
    tokenizer::TokenCounter,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    slice,
};

/// Replace complete source rows with one complete-table unit only when the verified counter fits.
#[expect(
    clippy::too_many_arguments,
    reason = "The packer consumes distinct document, mapping, graph, profile, and counter inputs."
)]
pub(super) fn pack_small_tables(
    input: &UnitGraphInput<'_>,
    mapped_index: &MappedTextIndex<'_>,
    index: &SourceIndex<'_>,
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
    let mut units_by_block = BTreeMap::<String, Vec<usize>>::new();
    for (unit_index, unit) in units.iter().enumerate() {
        units_by_block
            .entry(unit.source_block_id.clone())
            .or_default()
            .push(unit_index);
    }
    for table in tables {
        let source_blocks: BTreeSet<_> = index
            .descendants(&table.block_id)
            .iter()
            .filter(|block| matches!(block.block_type, BlockType::TableHead | BlockType::TableRow))
            .map(|block| block.block_id.as_str())
            .collect();
        let candidate_indices: Vec<_> = source_blocks
            .iter()
            .flat_map(|block_id| units_by_block.get(*block_id).into_iter().flatten().copied())
            .collect();
        if candidate_indices.is_empty() {
            continue;
        }
        let candidates: Vec<_> = candidate_indices
            .iter()
            .filter_map(|index| units.get(*index))
            .collect();
        let candidate_ids: BTreeSet<_> =
            candidates.iter().map(|unit| unit.unit_id.clone()).collect();
        let complete = complete_table_unit(table, &candidates, input)?;
        drop(candidates);
        if counter
            .token_ids(&prepared_text(&complete, mapped_index, &[], &[])?)?
            .len()
            <= size_limit(profile, UnitKind::Table)
        {
            units.retain(|unit| !candidate_ids.contains(&unit.unit_id));
            units.push(complete);
            units.sort_by_key(|unit| (first_start(&unit.parts), unit.unit_id.clone()));
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
    parts.sort_by_key(|part| first_start(slice::from_ref(part)));
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
        parent_id: None,
        parts,
        split: SplitMarker::Whole,
    })
}
