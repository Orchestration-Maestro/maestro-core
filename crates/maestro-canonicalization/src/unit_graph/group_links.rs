//! Reciprocal group links and source-ordered structural occurrences.
use super::types::{
    ContextRelationRecord, DeliveryUnit, Group, GroupKind, PartRole, SourcePart, UnitKind,
};
use std::collections::BTreeMap;

/// Reordinal primary group parts in source order.
pub(super) fn group_parts(parts: impl Iterator<Item = SourcePart>) -> Vec<SourcePart> {
    let mut parts: Vec<_> = parts
        .filter(|part| part.role == PartRole::Primary)
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
    parts
}

/// Make child lists and parent IDs reciprocal, ordered by each child's source position.
pub(super) fn wire_parentage(groups: &mut [Group], units: &mut [DeliveryUnit]) {
    for unit in units.iter_mut() {
        unit.parent_id = groups
            .iter()
            .filter(|group| group.children.contains(&unit.unit_id))
            .min_by_key(|group| parent_priority(group.kind))
            .map(|group| group.group_id.clone());
    }
    for group in groups.iter_mut() {
        group.children.clear();
    }
    let group_parents: BTreeMap<_, _> = groups
        .iter()
        .map(|group| (group.group_id.clone(), group.parent_id.clone()))
        .collect();
    let unit_parents: BTreeMap<_, _> = units
        .iter()
        .map(|unit| (unit.unit_id.clone(), unit.parent_id.clone()))
        .collect();
    let starts: BTreeMap<_, _> = groups
        .iter()
        .map(|group| {
            (
                group.group_id.clone(),
                group
                    .parts
                    .iter()
                    .flat_map(|part| &part.ranges)
                    .map(|range| range.start)
                    .min()
                    .unwrap_or(usize::MAX),
            )
        })
        .chain(units.iter().map(|unit| {
            (
                unit.unit_id.clone(),
                unit.parts
                    .iter()
                    .flat_map(|part| &part.ranges)
                    .map(|range| range.start)
                    .min()
                    .unwrap_or(usize::MAX),
            )
        }))
        .collect();
    for group in groups {
        group
            .children
            .extend(group_parents.iter().filter_map(|(child, parent)| {
                (parent.as_deref() == Some(group.group_id.as_str())).then_some(child.clone())
            }));
        group
            .children
            .extend(unit_parents.iter().filter_map(|(child, parent)| {
                (parent.as_deref() == Some(group.group_id.as_str())).then_some(child.clone())
            }));
        group.children.sort_by_key(|child| {
            (
                starts.get(child.as_str()).copied().unwrap_or(usize::MAX),
                child.clone(),
            )
        });
        group.children.dedup();
    }
}

/// Assign occurrence numbers by normalized structural ancestry in source order.
pub(super) fn assign_family_occurrences(groups: &mut [Group]) {
    let mut occurrences = BTreeMap::<Vec<String>, usize>::new();
    for group in groups {
        let occurrence = occurrences
            .entry(group.family.heading_path.clone())
            .or_default();
        group.family.occurrence = *occurrence;
        *occurrence += 1;
    }
}

/// Assign unit occurrences among same-kind siblings.
pub(super) fn assign_unit_occurrences(units: &mut [DeliveryUnit]) {
    let mut occurrences = BTreeMap::<(UnitKind, Option<String>, Vec<String>), usize>::new();
    for unit in units {
        let key = (
            unit.kind,
            unit.parent_id.clone(),
            unit.heading_path
                .iter()
                .map(|part| normalize(part))
                .collect(),
        );
        let occurrence = occurrences.entry(key).or_default();
        unit.occurrence = *occurrence;
        *occurrence += 1;
    }
}

/// Sort context relations by source byte position and assign group-local ordinals.
pub(super) fn order_context_relations(groups: &[Group], relations: &mut [ContextRelationRecord]) {
    relations.sort_by_key(|relation| {
        let start = groups
            .iter()
            .find(|group| group.group_id == relation.group_id)
            .and_then(|group| {
                group
                    .parts
                    .iter()
                    .find(|part| part.part_id == relation.part_id)
            })
            .and_then(|part| part.ranges.iter().map(|range| range.start).min())
            .unwrap_or(usize::MAX);
        (start, relation.group_id.clone(), relation.part_id.clone())
    });
    let mut previous: Option<String> = None;
    let mut ordinal = 0;
    for relation in relations {
        if previous.as_deref() == Some(relation.group_id.as_str()) {
            ordinal += 1;
        } else {
            previous = Some(relation.group_id.clone());
            ordinal = 0;
        }
        relation.ordinal = ordinal;
    }
}

/// Prefer the narrowest eligible structural group as a unit's direct parent.
fn parent_priority(kind: GroupKind) -> u8 {
    match kind {
        GroupKind::Row => 0,
        GroupKind::Table | GroupKind::Procedure | GroupKind::Code | GroupKind::Paragraphs => 1,
        GroupKind::Section => 2,
        GroupKind::Page => 3,
    }
}

/// Collapse heading whitespace and lowercase a family-key component.
fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
