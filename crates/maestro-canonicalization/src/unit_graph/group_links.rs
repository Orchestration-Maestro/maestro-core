//! Reciprocal group links and source-ordered structural occurrences.
use super::group_helpers::first_start;
use super::types::{ContextRelationRecord, DeliveryUnit, Group, GroupKind, PartRole, SourcePart};
use std::{
    collections::{BTreeMap, BTreeSet},
    slice,
};

/// Keep a group heading alongside its primary source parts in source order.
pub(super) fn group_parts_with_heading(
    parts: impl Iterator<Item = SourcePart>,
    heading: Option<SourcePart>,
) -> Vec<SourcePart> {
    let mut parts = group_parts(parts);
    if let Some(heading) = heading {
        parts.push(heading);
    }
    parts.sort_by_key(|part| first_start(slice::from_ref(part)));
    for (ordinal, part) in parts.iter_mut().enumerate() {
        part.ordinal = ordinal;
    }
    parts
}

/// Reordinal primary group parts in source order.
pub(super) fn group_parts(parts: impl Iterator<Item = SourcePart>) -> Vec<SourcePart> {
    let mut parts: Vec<_> = parts
        .filter(|part| part.role == PartRole::Primary)
        .collect();
    parts.sort_by_key(|part| first_start(slice::from_ref(part)));
    for (ordinal, part) in parts.iter_mut().enumerate() {
        part.ordinal = ordinal;
    }
    parts
}

/// Make child lists and parent IDs reciprocal, ordered by each child's source position.
pub(super) fn wire_parentage(groups: &mut [Group], units: &mut [DeliveryUnit]) {
    let unit_ids: BTreeSet<_> = units.iter().map(|unit| unit.unit_id.as_str()).collect();
    let mut parents = BTreeMap::<String, (u8, String)>::new();
    for group in groups.iter() {
        let priority = parent_priority(group.kind);
        for child in &group.children {
            if unit_ids.contains(child.as_str()) {
                let candidate = (priority, group.group_id.clone());
                parents
                    .entry(child.clone())
                    .and_modify(|parent| *parent = parent.clone().min(candidate.clone()))
                    .or_insert(candidate);
            }
        }
    }
    for unit in units.iter_mut() {
        unit.parent_id = parents.get(&unit.unit_id).map(|(_, parent)| parent.clone());
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
        .map(|group| (group.group_id.clone(), first_start(&group.parts)))
        .chain(
            units
                .iter()
                .map(|unit| (unit.unit_id.clone(), first_start(&unit.parts))),
        )
        .collect();
    let mut children_by_parent = BTreeMap::<String, Vec<String>>::new();
    for (child, parent) in group_parents.iter().chain(unit_parents.iter()) {
        if let Some(parent) = parent {
            children_by_parent
                .entry(parent.clone())
                .or_default()
                .push(child.clone());
        }
    }
    for group in groups {
        group.children = children_by_parent
            .remove(&group.group_id)
            .unwrap_or_default();
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
    let mut occurrences = BTreeMap::<(GroupKind, Vec<String>), usize>::new();
    for group in groups {
        if group.kind == GroupKind::Row {
            group.family.occurrence = 0;
            continue;
        }
        let occurrence = occurrences
            .entry((group.kind, group.family.heading_path.clone()))
            .or_default();
        group.family.occurrence = *occurrence;
        *occurrence += 1;
    }
}

/// Sort context relations by source byte position and assign group-local ordinals.
pub(super) fn order_context_relations(groups: &[Group], relations: &mut [ContextRelationRecord]) {
    let starts: BTreeMap<_, _> = groups
        .iter()
        .flat_map(|group| {
            group
                .parts
                .iter()
                .map(move |part| (part.part_id.as_str(), first_start(slice::from_ref(part))))
        })
        .collect();
    relations.sort_by_key(|relation| {
        (
            starts
                .get(relation.part_id.as_str())
                .copied()
                .unwrap_or(usize::MAX),
            relation.group_id.clone(),
            relation.part_id.clone(),
        )
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
