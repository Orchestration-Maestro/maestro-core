//! Retrieval preparation from ancestor headings and typed context relations.
use super::{
    group_helpers::first_start,
    profile::UnitProfile,
    types::{ContextRelationRecord, DeliveryUnit, Group, GroupKind, SourcePart, UnitKind},
};
use crate::{error::Error, source_units::MappedDocument};
use std::{
    collections::{BTreeMap, BTreeSet},
    slice,
};

/// Per-document graph lookups reused by size checks and ranking-view preparation.
pub(super) struct GraphIndex<'a> {
    /// Groups by stable identity.
    groups: BTreeMap<&'a str, &'a Group>,
    /// Unique graph parts by stable identity.
    parts: BTreeMap<&'a str, &'a SourcePart>,
    /// Typed context relations grouped by owner group.
    relations_by_group: BTreeMap<&'a str, Vec<&'a ContextRelationRecord>>,
    /// Delivery units by stable identity.
    units: BTreeMap<&'a str, &'a DeliveryUnit>,
    /// Delivery units by mapped source-unit identity.
    units_by_source_id: BTreeMap<&'a str, &'a DeliveryUnit>,
    /// Delivery-unit positions in source order.
    unit_positions: BTreeMap<&'a str, usize>,
}

impl<'a> GraphIndex<'a> {
    /// Return the delivery unit owning one mapped source unit.
    pub(super) fn unit_for_source(&self, source_id: &str) -> Option<&'a DeliveryUnit> {
        self.units_by_source_id.get(source_id).copied()
    }

    /// Return a delivery unit by stable identity.
    pub(super) fn unit(&self, unit_id: &str) -> Option<&'a DeliveryUnit> {
        self.units.get(unit_id).copied()
    }

    /// Return source-order position for one delivery unit.
    pub(super) fn unit_position(&self, unit_id: &str) -> Option<usize> {
        self.unit_positions.get(unit_id).copied()
    }

    /// Index graph identities once for one document.
    pub(super) fn new(
        units: &'a [DeliveryUnit],
        groups: &'a [Group],
        relations: &'a [ContextRelationRecord],
    ) -> Self {
        let parts: BTreeMap<_, _> = units
            .iter()
            .flat_map(|unit| &unit.parts)
            .chain(groups.iter().flat_map(|group| &group.parts))
            .map(|part| (part.part_id.as_str(), part))
            .collect();
        let groups_by_id: BTreeMap<_, _> = groups
            .iter()
            .map(|group| (group.group_id.as_str(), group))
            .collect();
        let mut units_by_id = BTreeMap::new();
        let mut units_by_source_id = BTreeMap::new();
        let mut unit_positions = BTreeMap::new();
        for (position, unit) in units.iter().enumerate() {
            units_by_id.insert(unit.unit_id.as_str(), unit);
            unit_positions.insert(unit.unit_id.as_str(), position);
            for source_id in unit
                .parts
                .iter()
                .flat_map(|part| &part.mappings)
                .map(|mapping| mapping.unit_id.as_str())
            {
                units_by_source_id.insert(source_id, unit);
            }
        }
        let mut relations_by_group = BTreeMap::<&str, Vec<_>>::new();
        for relation in relations {
            relations_by_group
                .entry(relation.group_id.as_str())
                .or_default()
                .push(relation);
        }
        Self {
            groups: groups_by_id,
            parts,
            relations_by_group,
            units: units_by_id,
            units_by_source_id,
            unit_positions,
        }
    }
}

/// Mapped canonical text lookup reused while preparing all units.
pub(super) struct MappedTextIndex<'a> {
    /// Canonical mapped text by source-unit identity.
    texts: BTreeMap<&'a str, &'a str>,
}

impl<'a> MappedTextIndex<'a> {
    /// Index source text once for one mapped document.
    pub(super) fn new(mapped: &'a MappedDocument) -> Self {
        Self {
            texts: mapped
                .units
                .iter()
                .map(|unit| (unit.unit_id.as_str(), unit.text.as_str()))
                .collect(),
        }
    }
}

/// Resolve heading parts from section and page ancestors.
pub(super) fn unit_heading_parts_indexed(
    unit: &DeliveryUnit,
    index: &GraphIndex<'_>,
) -> Result<Vec<SourcePart>, Error> {
    let mut ancestors = Vec::new();
    let mut visited = BTreeSet::new();
    let mut parent = unit.parent_id.as_deref();
    while let Some(group_id) = parent {
        if !visited.insert(group_id) {
            return Err(Error(
                "delivery unit group ancestry contains a cycle".into(),
            ));
        }
        let group = index
            .groups
            .get(group_id)
            .ok_or_else(|| Error("delivery unit has an unknown group ancestor".into()))?;
        ancestors.push(*group);
        parent = group.parent_id.as_deref();
    }
    ancestors.reverse();
    let mut headings = Vec::new();
    let mut seen = BTreeSet::new();
    for group in ancestors {
        if matches!(group.kind, GroupKind::Page | GroupKind::Section)
            && let Some(heading_id) = group.heading.as_deref()
            && seen.insert(heading_id)
        {
            let part = index
                .parts
                .get(heading_id)
                .ok_or_else(|| Error("group heading references an unowned source part".into()))?;
            headings.push((*part).clone());
        }
    }
    Ok(headings)
}

/// Resolve unique required context and ancestor headings in source order.
pub(super) fn unit_all_context_parts_indexed(
    unit: &DeliveryUnit,
    index: &GraphIndex<'_>,
) -> Result<Vec<SourcePart>, Error> {
    let mut parts = unit_context_parts_indexed(unit, index)?;
    parts.extend(unit_heading_parts_indexed(unit, index)?);
    parts.sort_by_key(|part| (first_start(slice::from_ref(part)), part.part_id.clone()));
    let mut seen = BTreeSet::new();
    parts.retain(|part| seen.insert(part.part_id.clone()));
    Ok(parts)
}

/// Resolve typed context only from relations on structural ancestors.
pub(super) fn unit_context_parts_indexed(
    unit: &DeliveryUnit,
    index: &GraphIndex<'_>,
) -> Result<Vec<SourcePart>, Error> {
    let primary_ids: BTreeSet<_> = unit
        .parts
        .iter()
        .map(|part| part.part_id.as_str())
        .collect();
    let mut context_ids = BTreeSet::new();
    let mut contexts = Vec::new();
    let mut visited = BTreeSet::new();
    let mut parent = unit.parent_id.as_deref();
    while let Some(group_id) = parent {
        if !visited.insert(group_id) {
            return Err(Error(
                "delivery unit group ancestry contains a cycle".into(),
            ));
        }
        let group = index
            .groups
            .get(group_id)
            .ok_or_else(|| Error("delivery unit has an unknown group ancestor".into()))?;
        for relation in index.relations_by_group.get(group_id).into_iter().flatten() {
            if primary_ids.contains(relation.part_id.as_str())
                || !context_ids.insert(relation.part_id.as_str())
            {
                continue;
            }
            let part = index.parts.get(relation.part_id.as_str()).ok_or_else(|| {
                Error("context relation references an unowned source part".into())
            })?;
            contexts.push((*part).clone());
        }
        parent = group.parent_id.as_deref();
    }
    contexts.sort_by_key(|part| (first_start(slice::from_ref(part)), part.part_id.clone()));
    Ok(contexts)
}

/// Render actual heading and source-mapped context plus primary contributions.
pub(super) fn prepared_text(
    unit: &DeliveryUnit,
    mapped: &MappedTextIndex<'_>,
    contexts: &[SourcePart],
    heading_parts: &[SourcePart],
) -> Result<String, Error> {
    let body = mapped_parts_text(&unit.parts, mapped)?;
    let context = mapped_parts_text(contexts, mapped)?;
    let headings = mapped_parts_text(heading_parts, mapped)?;
    let headings = if headings.is_empty() {
        unit.heading_path.join("\n")
    } else {
        headings.replace('\n', " / ")
    };
    Ok([headings, context, body]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n"))
}

/// Resolve unique mapped source text in part and mapping order.
fn mapped_parts_text(parts: &[SourcePart], mapped: &MappedTextIndex<'_>) -> Result<String, Error> {
    let mut seen = BTreeSet::new();
    let mut pieces = Vec::new();
    for mapping in parts.iter().flat_map(|part| &part.mappings) {
        if seen.insert(mapping.unit_id.as_str()) {
            pieces.push(*mapped.texts.get(mapping.unit_id.as_str()).ok_or_else(|| {
                Error("delivery unit mapping names a missing source unit".into())
            })?);
        }
    }
    Ok(pieces.join("\n"))
}

/// Select the profile's provisional verified-counter limit for one unit kind.
pub(super) fn size_limit(profile: UnitProfile, kind: UnitKind) -> usize {
    let limits = profile.size_limits();
    match kind {
        UnitKind::Section => limits.section_tokens,
        UnitKind::Table => limits.table_tokens,
        UnitKind::Row => limits.row_tokens,
        UnitKind::Procedure => limits.procedure_tokens,
        UnitKind::Code => limits.code_tokens,
        UnitKind::Paragraphs | UnitKind::Block => limits.paragraphs_tokens,
    }
}
