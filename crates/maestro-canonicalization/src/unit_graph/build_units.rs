//! Assemble complete delivery units from primary canonical mappings.
use super::types::{
    DeliveryUnit, MappingContribution, PartRole, SourcePart, SourceRange, SplitMarker,
    UnitGraphInput, UnitKind,
};
use crate::{
    content::BlockType,
    error::Error,
    hashing::digest,
    source_units::{MappedDocument, OriginMode, SourceUnit},
};
use serde::Serialize;
use std::collections::BTreeMap;

/// Identity input for a source part.
#[derive(Serialize)]
struct PartIdentity<'a> {
    /// Record version.
    version: &'static str,
    /// Source-unit ID.
    unit_id: &'a str,
    /// Part source ranges.
    ranges: &'a [SourceRange],
}

/// Build whole canonical block units, combining inline contributions of a paragraph or row.
pub(super) fn build_units(
    input: &UnitGraphInput<'_>,
    mapped: &MappedDocument,
) -> Result<Vec<DeliveryUnit>, Error> {
    let blocks: BTreeMap<_, _> = input
        .document
        .blocks
        .iter()
        .map(|block| (block.block_id.as_str(), block))
        .collect();
    let mut by_block: BTreeMap<String, Vec<&SourceUnit>> = BTreeMap::new();
    for unit in mapped.units.iter().filter(|unit| unit.primary) {
        let owner = blocks
            .get(unit.block_id.as_str())
            .copied()
            .ok_or_else(|| Error("mapped unit names an unknown block".into()))?;
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
                occurrence: 0,
                parent_id: block
                    .parent_block_id
                    .as_ref()
                    .map(|parent| format!("block-{parent}")),
                parts,
                split: SplitMarker::Whole,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    result.sort_by_key(|unit| {
        unit.parts
            .iter()
            .flat_map(|part| &part.ranges)
            .map(|range| range.start)
            .min()
            .unwrap_or(usize::MAX)
    });
    for (ordinal, unit) in result.iter_mut().enumerate() {
        unit.occurrence = ordinal;
    }
    Ok(result)
}

/// Map one complete canonical contribution to original UTF-8 ranges.
fn source_part(unit: &SourceUnit, ordinal: usize) -> Result<SourcePart, Error> {
    let mut contributions: Vec<_> = unit
        .mappings
        .iter()
        .flat_map(|mapping| {
            mapping.origins.iter().map(move |origin| {
                (
                    SourceRange {
                        start: origin.span.start,
                        end: origin.span.end,
                    },
                    MappingContribution {
                        unit_id: unit.unit_id.clone(),
                        derived_range: (mapping.range.start, mapping.range.end),
                        mapping_mode: mapping_name(mapping.mode).into(),
                    },
                )
            })
        })
        .collect();
    contributions.sort_by_key(|(range, mapping)| (range.start, range.end, mapping.derived_range));
    let ranges: Vec<_> = contributions.iter().map(|(range, _)| *range).collect();
    if ranges.is_empty()
        || ranges.iter().any(|range| range.start >= range.end)
        || ranges.windows(2).any(|pair| {
            pair.first()
                .zip(pair.get(1))
                .is_some_and(|(left, right)| left.end > right.start)
        })
    {
        return Err(Error(
            "primary delivery unit has invalid source mapping".into(),
        ));
    }
    let part_identity = serde_json::to_vec(&PartIdentity {
        version: "unit-part/1",
        unit_id: &unit.unit_id,
        ranges: &ranges,
    })
    .map_err(|error| Error(error.to_string()))?;
    let mappings = contributions
        .into_iter()
        .map(|(_, mapping)| mapping)
        .collect();
    Ok(SourcePart {
        part_id: format!("part-{}", digest(&part_identity)),
        role: PartRole::Primary,
        ordinal,
        ranges,
        mappings,
    })
}

/// Return the stable name pinned by the mapping artifact wire schema.
fn mapping_name(mode: OriginMode) -> &'static str {
    match mode {
        OriginMode::ExactCopy => "exact",
        OriginMode::CanonicalTransformation => "canonical_transformation",
        OriginMode::Formatting => "formatting",
    }
}

/// Assign a delivery unit kind from its canonical block type.
fn kind(block_type: &BlockType) -> UnitKind {
    match block_type {
        BlockType::Table | BlockType::TableRow => UnitKind::Row,
        BlockType::Code => UnitKind::Code,
        BlockType::ListItem => UnitKind::Procedure,
        BlockType::Heading => UnitKind::Section,
        BlockType::Paragraph => UnitKind::Paragraphs,
        _ => UnitKind::Block,
    }
}
