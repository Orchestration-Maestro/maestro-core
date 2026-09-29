//! Reciprocal ancestry and context-reference validation for graph groups.
use super::types::{DeliveryGraph, PartRole, SourcePart};
use crate::error::Error;
use std::collections::{BTreeMap, BTreeSet};

/// Check every group has an identity, valid children and ordered parts.
pub(super) fn validate_groups(
    graph: &DeliveryGraph,
    canonical_parts: &BTreeMap<String, SourcePart>,
) -> Result<(), Error> {
    let groups: BTreeMap<_, _> = graph
        .groups
        .iter()
        .map(|group| (group.group_id.as_str(), group))
        .collect();
    let units: BTreeMap<_, _> = graph
        .units
        .iter()
        .map(|unit| (unit.unit_id.as_str(), unit))
        .collect();
    if groups.len() != graph.groups.len() || groups.keys().any(|id| units.contains_key(id)) {
        return Err(Error("duplicate graph group identity".into()));
    }
    validate_group_nodes(graph, &groups, &units, canonical_parts)?;
    validate_unit_parents(graph, &groups)?;
    validate_group_cycles(graph, &groups)?;
    validate_context_relations(graph, &groups, canonical_parts)
}

/// Check group identities, links and source references.
fn validate_group_nodes(
    graph: &DeliveryGraph,
    groups: &BTreeMap<&str, &super::types::Group>,
    units: &BTreeMap<&str, &super::types::DeliveryUnit>,
    canonical_parts: &BTreeMap<String, SourcePart>,
) -> Result<(), Error> {
    let mut known_ids: BTreeSet<_> = units.keys().copied().collect();
    known_ids.extend(groups.keys().copied());
    for group in &graph.groups {
        if group.group_id.trim().is_empty() {
            return Err(Error("group ID is empty".into()));
        }
        if group
            .children
            .iter()
            .any(|child| !known_ids.contains(child.as_str()))
            || group.children.iter().collect::<BTreeSet<_>>().len() != group.children.len()
        {
            return Err(Error("group has dangling or duplicate child".into()));
        }
        match &group.parent_id {
            Some(parent_id) => {
                let parent = groups
                    .get(parent_id.as_str())
                    .ok_or_else(|| Error("group has unknown parent".into()))?;
                if !valid_group_parent(group.kind, parent.kind) {
                    return Err(Error(
                        "group parent kind is not allowed by the ancestry contract".into(),
                    ));
                }
                if !parent.children.contains(&group.group_id) {
                    return Err(Error("group parent link is not reciprocal".into()));
                }
            }
            None if group.kind != super::types::GroupKind::Page => {
                return Err(Error("non-page group has no parent".into()));
            }
            None => {}
        }
        if group.children.iter().any(|child| {
            let parent_id = groups
                .get(child.as_str())
                .map(|child| child.parent_id.as_deref())
                .or_else(|| {
                    units
                        .get(child.as_str())
                        .map(|child| child.parent_id.as_deref())
                });
            parent_id.flatten() != Some(group.group_id.as_str())
        }) {
            return Err(Error("group child link is not reciprocal".into()));
        }
        for part in &group.parts {
            let primary = canonical_parts
                .get(&part.part_id)
                .filter(|owner| owner.role == PartRole::Primary)
                .ok_or_else(|| Error("group part does not reference primary ownership".into()))?;
            if part.ranges != primary.ranges || part.mappings != primary.mappings {
                return Err(Error(
                    "group part differs from primary source mapping".into(),
                ));
            }
        }
        if group
            .parts
            .iter()
            .enumerate()
            .any(|(ordinal, part)| part.ordinal != ordinal)
        {
            return Err(Error("group part order is not contiguous".into()));
        }
    }
    Ok(())
}

/// Check each delivery unit has a parent of its allowed structural kind.
fn validate_unit_parents(
    graph: &DeliveryGraph,
    groups: &BTreeMap<&str, &super::types::Group>,
) -> Result<(), Error> {
    for unit in &graph.units {
        let parent_id = unit
            .parent_id
            .as_deref()
            .ok_or_else(|| Error("unit has no structural parent".into()))?;
        let parent = groups
            .get(parent_id)
            .ok_or_else(|| Error("unit has unknown parent".into()))?;
        if !valid_unit_parent(unit.kind, parent.kind) {
            return Err(Error(
                "unit parent kind is not allowed by the ancestry contract".into(),
            ));
        }
        if !parent.children.contains(&unit.unit_id) {
            return Err(Error("unit parent link is not reciprocal".into()));
        }
    }
    Ok(())
}

/// Refuse cycles in all group ancestry chains.
fn validate_group_cycles(
    graph: &DeliveryGraph,
    groups: &BTreeMap<&str, &super::types::Group>,
) -> Result<(), Error> {
    for group in &graph.groups {
        let mut seen = BTreeSet::new();
        let mut parent = group.parent_id.as_deref();
        while let Some(id) = parent {
            if !seen.insert(id) || id == group.group_id {
                return Err(Error("group ancestry contains a cycle".into()));
            }
            parent = groups.get(id).and_then(|group| group.parent_id.as_deref());
        }
    }
    Ok(())
}

/// Validate typed relations against owning structural groups and canonical parts.
fn validate_context_relations(
    graph: &DeliveryGraph,
    groups: &BTreeMap<&str, &super::types::Group>,
    canonical_parts: &BTreeMap<String, SourcePart>,
) -> Result<(), Error> {
    let mut relation_ordinals = BTreeMap::<&str, usize>::new();
    for relation in &graph.context_relations {
        let group = groups
            .get(relation.group_id.as_str())
            .ok_or_else(|| Error("context relation has unknown group".into()))?;
        if matches!(
            relation.kind,
            super::types::ContextRelation::HeaderToTable
                | super::types::ContextRelation::CaptionToTable
        ) && group.kind != super::types::GroupKind::Table
        {
            return Err(Error(
                "table context relation targets a non-table group".into(),
            ));
        }
        let part = canonical_parts
            .get(&relation.part_id)
            .filter(|part| part.role == PartRole::Primary)
            .ok_or_else(|| Error("context relation does not reference primary ownership".into()))?;
        if !group.parts.iter().any(|listed| {
            listed.part_id == relation.part_id
                && listed.ranges == part.ranges
                && listed.mappings == part.mappings
        }) {
            return Err(Error(
                "context relation part is not listed in its group".into(),
            ));
        }
        let ordinal = relation_ordinals.entry(&relation.group_id).or_default();
        if relation.ordinal != *ordinal {
            return Err(Error("context relation ordinal is not contiguous".into()));
        }
        *ordinal += 1;
    }
    Ok(())
}

/// Check a group parent edge against the structural ancestry rules.
fn valid_group_parent(child: super::types::GroupKind, parent: super::types::GroupKind) -> bool {
    use super::types::GroupKind::{Code, Page, Paragraphs, Procedure, Row, Section, Table};
    match child {
        Page => false,
        Row => parent == Table,
        Table | Procedure | Code | Paragraphs => matches!(parent, Section | Page),
        Section => matches!(parent, Section | Page),
    }
}

/// Check a delivery-unit parent edge against the structural ancestry rules.
fn valid_unit_parent(child: super::types::UnitKind, parent: super::types::GroupKind) -> bool {
    use super::types::{GroupKind, UnitKind};
    match child {
        UnitKind::Row => parent == GroupKind::Row,
        UnitKind::Table => parent == GroupKind::Table,
        UnitKind::Procedure => parent == GroupKind::Procedure,
        UnitKind::Section => parent == GroupKind::Section,
        UnitKind::Code => parent == GroupKind::Code,
        UnitKind::Paragraphs => matches!(parent, GroupKind::Section | GroupKind::Page),
        UnitKind::Block => matches!(
            parent,
            GroupKind::Row
                | GroupKind::Table
                | GroupKind::Procedure
                | GroupKind::Paragraphs
                | GroupKind::Section
                | GroupKind::Page
        ),
    }
}
