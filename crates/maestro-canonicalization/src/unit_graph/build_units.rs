//! Assemble complete delivery units from primary canonical mappings.
use super::{
    group_helpers::{first_start, source_part},
    types::{DeliveryUnit, SplitMarker, UnitGraphInput, UnitKind},
};
use crate::{
    content::BlockType,
    error::Error,
    hashing::digest,
    source_units::{MappedDocument, SourceUnit},
};
use std::collections::{BTreeMap, BTreeSet};

/// Build whole canonical block units, combining inline contributions of a paragraph or row.
pub(super) fn build_units(
    input: &UnitGraphInput<'_>,
    mapped: &MappedDocument,
    excluded_source_units: &BTreeSet<String>,
) -> Result<Vec<DeliveryUnit>, Error> {
    let blocks: BTreeMap<_, _> = input
        .document
        .blocks
        .iter()
        .map(|block| (block.block_id.as_str(), block))
        .collect();
    let mut by_block: BTreeMap<String, Vec<&SourceUnit>> = BTreeMap::new();
    for unit in mapped
        .units
        .iter()
        .filter(|unit| unit.primary && !excluded_source_units.contains(&unit.unit_id))
    {
        let owner = blocks
            .get(unit.block_id.as_str())
            .copied()
            .ok_or_else(|| Error("mapped unit names an unknown block".into()))?;
        if owner.block_type == BlockType::Heading {
            continue;
        }
        let target = if owner.block_type == BlockType::TableCell {
            owner
                .parent_block_id
                .as_deref()
                .and_then(|id| blocks.get(id).copied())
                .ok_or_else(|| Error("table cell has no row parent".into()))?
        } else {
            owner
        };
        by_block
            .entry(target.block_id.clone())
            .or_default()
            .push(unit);
    }
    let mut result = by_block
        .into_iter()
        .map(|(block_id, units)| {
            let block = blocks
                .get(block_id.as_str())
                .copied()
                .ok_or_else(|| Error("delivery block is missing".into()))?;
            let mut parts = Vec::new();
            for (ordinal, unit) in units.iter().enumerate() {
                parts.push(source_part(unit, ordinal)?);
            }
            let mut identity_units: Vec<_> =
                units.iter().map(|unit| unit.unit_id.as_str()).collect();
            identity_units.sort_unstable();
            let identity = serde_json::to_vec(&(
                "delivery-unit/1",
                &input.document.document_id,
                &input.document.revision_id,
                &block_id,
                &identity_units,
            ))
            .map_err(|error| Error(error.to_string()))?;
            let unit_id = format!("unit-{}", digest(&identity));
            let kind = kind(&block.block_type);
            Ok(DeliveryUnit {
                unit_id,
                kind,
                source_block_id: block.block_id.clone(),
                section_id: block.parent_section_id.clone(),
                heading_path: block.heading_path.clone(),
                parent_id: None,
                parts,
                split: SplitMarker::Whole,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    result.sort_by_key(|unit| first_start(&unit.parts));
    Ok(result)
}

/// Assign a delivery unit kind from its canonical block type.
fn kind(block_type: &BlockType) -> UnitKind {
    match block_type {
        BlockType::Table | BlockType::TableRow => UnitKind::Row,
        BlockType::Code => UnitKind::Code,
        BlockType::ListItem => UnitKind::Procedure,
        BlockType::Paragraph => UnitKind::Paragraphs,
        _ => UnitKind::Block,
    }
}
