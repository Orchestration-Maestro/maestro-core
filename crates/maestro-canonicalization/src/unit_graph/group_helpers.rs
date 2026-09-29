//! Canonical source ancestry and stable identifiers for graph groups.
use super::types::{
    DeliveryUnit, MappingContribution, PartRole, SourcePart, SourceRange, UnitGraphInput,
};
use crate::{
    content::{Block, BlockType},
    error::Error,
    hashing::digest,
    source_units::{MappedDocument, OriginMode, SourceUnit},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Per-document block, source-unit and ancestry indexes shared by graph builders.
pub(super) struct SourceIndex<'a> {
    /// Canonical blocks by stable identity.
    blocks: BTreeMap<String, &'a Block>,
    /// Mapped source units by stable identity.
    mapped_units: BTreeMap<String, &'a SourceUnit>,
    /// Primary mapped units grouped by source block.
    primary_units_by_block: BTreeMap<String, Vec<&'a SourceUnit>>,
    /// Transitive ancestor IDs for each source block.
    ancestors: BTreeMap<String, BTreeSet<String>>,
    /// Source block IDs from each block through its root.
    lineage: BTreeMap<String, Vec<String>>,
    /// Canonical block kinds by stable identity.
    block_types: BTreeMap<String, BlockType>,
    /// Immediate preceding sibling block IDs.
    previous_siblings: BTreeMap<String, String>,
    /// Descendant blocks in canonical source order.
    descendants: BTreeMap<String, Vec<&'a Block>>,
}

impl<'a> SourceIndex<'a> {
    /// Build block and source-unit indexes once for one mapped document.
    pub(super) fn new(input: &UnitGraphInput<'a>, mapped: &'a MappedDocument) -> Self {
        let blocks: BTreeMap<_, _> = input
            .document
            .blocks
            .iter()
            .map(|block| (block.block_id.clone(), block))
            .collect();
        let mapped_units = mapped
            .units
            .iter()
            .map(|unit| (unit.unit_id.clone(), unit))
            .collect();
        let mut primary_units_by_block = BTreeMap::<String, Vec<_>>::new();
        for unit in mapped.units.iter().filter(|unit| unit.primary) {
            primary_units_by_block
                .entry(unit.block_id.clone())
                .or_default()
                .push(unit);
        }
        let mut ancestors = BTreeMap::new();
        let mut lineage = BTreeMap::new();
        let mut previous_siblings = BTreeMap::new();
        let mut last_sibling = BTreeMap::<Option<String>, String>::new();
        for block in &input.document.blocks {
            let parent = block.parent_block_id.clone();
            if let Some(previous) = last_sibling.get(&parent) {
                previous_siblings.insert(block.block_id.clone(), previous.clone());
            }
            last_sibling.insert(parent, block.block_id.clone());
        }
        let block_types = input
            .document
            .blocks
            .iter()
            .map(|block| (block.block_id.clone(), block.block_type.clone()))
            .collect();
        let mut descendants = BTreeMap::<String, Vec<&Block>>::new();
        for block in &input.document.blocks {
            let mut chain = vec![block.block_id.clone()];
            let mut parent = block.parent_block_id.as_deref();
            while let Some(parent_block) = parent.and_then(|parent_id| blocks.get(parent_id)) {
                chain.push(parent_block.block_id.clone());
                parent = parent_block.parent_block_id.as_deref();
            }
            let ancestor_set: BTreeSet<_> = chain.iter().cloned().collect();
            for ancestor in &chain {
                descendants.entry(ancestor.clone()).or_default().push(block);
            }
            ancestors.insert(block.block_id.clone(), ancestor_set);
            lineage.insert(block.block_id.clone(), chain);
        }
        Self {
            blocks,
            mapped_units,
            primary_units_by_block,
            ancestors,
            lineage,
            block_types,
            previous_siblings,
            descendants,
        }
    }

    /// Return one primary canonical source unit by its owning block.
    pub(super) fn primary_unit_for_block(&self, block_id: &str) -> Option<&'a SourceUnit> {
        self.primary_units_by_block
            .get(block_id)
            .and_then(|units| units.first().copied())
    }

    /// Return a canonical block by identity.
    pub(super) fn block(&self, block_id: &str) -> Option<&'a Block> {
        self.blocks.get(block_id).copied()
    }

    /// Return the immediately preceding sibling of a canonical block.
    pub(super) fn previous_sibling(&self, block_id: &str) -> Option<&'a Block> {
        self.previous_siblings
            .get(block_id)
            .and_then(|previous| self.block(previous))
    }

    /// Return canonical blocks equal to or below an ancestor, in document order.
    pub(super) fn descendants(&self, block_id: &str) -> &[&'a Block] {
        self.descendants
            .get(block_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Check ancestry using the precomputed source block chain.
    pub(super) fn is_below(&self, block_id: &str, ancestor_id: &str) -> bool {
        self.ancestors
            .get(block_id)
            .is_some_and(|chain| chain.contains(ancestor_id))
    }

    /// Check whether any part mapping originates below an ancestor.
    pub(super) fn part_maps_under(&self, part: &SourcePart, ancestor_id: &str) -> bool {
        part.mappings.iter().any(|mapping| {
            self.mapped_units
                .get(&mapping.unit_id)
                .is_some_and(|unit| self.is_below(&unit.block_id, ancestor_id))
        })
    }

    /// Return the block's ancestor chain from itself to its root.
    pub(super) fn lineage(&self, block_id: &str) -> &[String] {
        self.lineage
            .get(block_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Check whether a list has an enclosing list and should fold into it.
    pub(super) fn nested_list(&self, block_id: &str) -> bool {
        self.lineage(block_id)
            .iter()
            .skip(1)
            .any(|ancestor| self.block_types.get(ancestor) == Some(&BlockType::List))
    }

    /// Check whether a block is a table or code boundary.
    pub(super) fn is_table_or_code(&self, block_id: &str) -> bool {
        matches!(
            self.block_types.get(block_id),
            Some(BlockType::Table | BlockType::Code)
        )
    }

    /// Return each ancestor of a source block, including itself.
    pub(super) fn ancestors_of(&self, block_id: &str) -> impl Iterator<Item = &str> {
        self.ancestors
            .get(block_id)
            .into_iter()
            .flat_map(|ancestors| ancestors.iter().map(String::as_str))
    }
}

/// Map one complete canonical contribution to original UTF-8 ranges.
pub(super) fn source_part(unit: &SourceUnit, ordinal: usize) -> Result<SourcePart, Error> {
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
    let identity_ranges = ranges
        .iter()
        .map(|range| format!("[{},{}]", range.start, range.end))
        .collect::<Vec<_>>()
        .join(",");
    let identity_unit =
        serde_json::to_string(&unit.unit_id).map_err(|error| Error(error.to_string()))?;
    let part_identity = format!(
        "{{\"version\":\"unit-part/1\",\"unit_id\":{identity_unit},\"ranges\":[{identity_ranges}]}}"
    );
    let mappings = contributions
        .into_iter()
        .map(|(_, mapping)| mapping)
        .collect();
    Ok(SourcePart {
        part_id: format!("part-{}", digest(part_identity.as_bytes())),
        role: PartRole::Primary,
        ordinal,
        ranges,
        mappings,
    })
}

/// Return the stable mapping name for one source-origin mode.
fn mapping_name(mode: OriginMode) -> &'static str {
    match mode {
        OriginMode::ExactCopy => "exact",
        OriginMode::CanonicalTransformation => "canonical_transformation",
        OriginMode::Formatting => "formatting",
    }
}

/// Build the non-unit source part represented by one canonical heading block.
pub(super) fn heading_part(
    index: &SourceIndex<'_>,
    source_block_id: &str,
) -> Result<Option<SourcePart>, Error> {
    let Some(unit) = index.primary_unit_for_block(source_block_id) else {
        return Ok(None);
    };
    let mut part = source_part(unit, 0)?;
    part.role = PartRole::RequiredContext;
    Ok(Some(part))
}

/// Build the sorted source contributions for one row group.
pub(super) fn row_parts(
    row_units: &[&DeliveryUnit],
    row: &Block,
    table_parts: &[SourcePart],
    index: &SourceIndex<'_>,
) -> Vec<SourcePart> {
    if row_units.is_empty() {
        table_parts
            .iter()
            .filter(|part| index.part_maps_under(part, &row.block_id))
            .cloned()
            .collect()
    } else {
        row_units
            .iter()
            .flat_map(|unit| unit.parts.iter().cloned())
            .collect()
    }
}

/// The immediately preceding paragraph sibling, used as a real lead-in.
pub(super) fn preceding_lead_in<'a>(index: &SourceIndex<'a>, block: &Block) -> Option<&'a Block> {
    index
        .previous_sibling(&block.block_id)
        .filter(|candidate| candidate.block_type == BlockType::Paragraph)
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

/// Return the earliest source byte owned by a set of parts.
pub(super) fn first_start(parts: &[SourcePart]) -> usize {
    parts
        .iter()
        .flat_map(|part| &part.ranges)
        .map(|range| range.start)
        .min()
        .unwrap_or(usize::MAX)
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
