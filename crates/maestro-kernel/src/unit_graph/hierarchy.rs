//! Structural ancestry and context derivation without transitive storage.
use super::{
    error::{Error, require},
    types::{ContextKind, DeliveryGraph, DeliveryUnit, Group, GroupKind, UnitKind},
};
use std::collections::{BTreeMap, BTreeSet};

/// Checks unique identities, reciprocal edges, structural kinds and contexts.
pub(super) fn validate(graph: &DeliveryGraph) -> Result<(), Error> {
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
    require(
        groups.len() == graph.groups.len()
            && units.len() == graph.units.len()
            && groups.keys().all(|id| !units.contains_key(id)),
        "duplicate graph identity",
    )?;
    let part_order: BTreeMap<_, _> = graph
        .parts
        .iter()
        .enumerate()
        .map(|(index, part)| (part.part_id.as_str(), index))
        .collect();
    let mut children = BTreeSet::new();
    for group in &graph.groups {
        validate_family(graph, group, &groups)?;
        validate_children(group, &groups, &units, &mut children)?;
        validate_context(group, &part_order)?;
        validate_heading(group)?;
        validate_group_parts(group)?;
        if group.kind == GroupKind::Code {
            require(
                group.children.iter().any(|child| {
                    units
                        .get(child.as_str())
                        .is_some_and(|unit| unit.kind == UnitKind::Code)
                }),
                "code group requires a code child unit",
            )?;
            require(
                group
                    .context_relations
                    .iter()
                    .any(|relation| relation.kind == ContextKind::LeadIn),
                "code group requires a lead_in relation",
            )?;
        }
    }
    validate_units(graph, &groups, &children)?;
    validate_acyclic(&groups)
}

/// Validates family identity and structural parent-kind rules.
fn validate_family(
    graph: &DeliveryGraph,
    group: &Group,
    groups: &BTreeMap<&str, &Group>,
) -> Result<(), Error> {
    require(
        group.family.collection_id == graph.descriptor.collection_id
            && group.family.source_namespace == graph.descriptor.source_namespace
            && !group.family.path_segment.trim().is_empty()
            && !group.family.page_title.trim().is_empty(),
        "invalid structural family",
    )?;
    let parent_valid = match (group.kind, group.parent.as_deref()) {
        (GroupKind::Row, Some(parent)) => group_kind_is(parent, GroupKind::Table, &[], groups),
        (
            GroupKind::Table | GroupKind::Section | GroupKind::Code | GroupKind::Procedure,
            Some(parent),
        ) => group_kind_is(parent, GroupKind::Section, &[GroupKind::Page], groups),
        (GroupKind::Page, None) | (GroupKind::Paragraphs, _) => true,
        _ => false,
    };
    require(parent_valid, "invalid structural parent kind")
}

/// Checks a group's kind against one allowed parent identity.
fn group_kind_is(
    id: &str,
    primary: GroupKind,
    alternatives: &[GroupKind],
    groups: &BTreeMap<&str, &Group>,
) -> bool {
    groups
        .get(id)
        .is_some_and(|group| group.kind == primary || alternatives.contains(&group.kind))
}

/// Validates that each listed child points back to this group exactly once.
fn validate_children(
    group: &Group,
    groups: &BTreeMap<&str, &Group>,
    units: &BTreeMap<&str, &DeliveryUnit>,
    children: &mut BTreeSet<String>,
) -> Result<(), Error> {
    for child in &group.children {
        let parent = groups
            .get(child.as_str())
            .map(|parent_group| parent_group.parent.as_deref())
            .or_else(|| units.get(child.as_str()).map(|unit| unit.parent.as_deref()));
        require(
            parent == Some(Some(group.group_id.as_str())) && children.insert(child.clone()),
            "nonreciprocal or duplicate parent edge",
        )?;
    }
    if let Some(parent_id) = group.parent.as_deref() {
        require(
            groups.contains_key(parent_id) && children.contains(&group.group_id),
            "missing parent edge",
        )?;
    } else if group.kind != GroupKind::Page {
        require(
            matches!(group.kind, GroupKind::Procedure | GroupKind::Paragraphs),
            "missing structural parent",
        )?;
    }
    Ok(())
}

/// Checks typed direct context relations and their ordering.
fn validate_context(group: &Group, part_order: &BTreeMap<&str, usize>) -> Result<(), Error> {
    require(
        group.context_relations.windows(2).all(|pair| match pair {
            [first, second] => {
                part_order.get(first.part_id.as_str()) < part_order.get(second.part_id.as_str())
            }
            _ => true,
        }),
        "context relations are not in source order",
    )?;
    let mut seen = BTreeSet::new();
    for relation in &group.context_relations {
        require(
            group.part_ids.contains(&relation.part_id) && seen.insert(relation.part_id.as_str()),
            "context must reference a unique direct part",
        )?;
        match relation.kind {
            ContextKind::HeaderToTable | ContextKind::CaptionToTable => require(
                group.kind == GroupKind::Table,
                "table context requires table group",
            )?,
            ContextKind::LeadIn => require(
                matches!(
                    group.kind,
                    GroupKind::Procedure | GroupKind::Code | GroupKind::Section
                ),
                "lead_in requires procedure, code or section group",
            )?,
        }
    }
    Ok(())
}

/// Validates heading placement and the allowed group-owned part roles.
fn validate_heading(group: &Group) -> Result<(), Error> {
    if let Some(heading) = &group.heading {
        require(
            matches!(group.kind, GroupKind::Page | GroupKind::Section)
                && group.part_ids.first() == Some(heading),
            "invalid group heading",
        )?;
    }
    Ok(())
}

/// Refuses direct group parts that are neither headings nor typed context.
fn validate_group_parts(group: &Group) -> Result<(), Error> {
    let context: BTreeSet<_> = group
        .context_relations
        .iter()
        .map(|relation| relation.part_id.as_str())
        .collect();
    require(
        group.part_ids.iter().all(|part| {
            group
                .heading
                .as_ref()
                .is_some_and(|heading| heading == part)
                || context.contains(part.as_str())
        }),
        "unsearchable direct group part",
    )
}

/// Checks unit-to-group reciprocity and the code-specific child relation.
fn validate_units(
    graph: &DeliveryGraph,
    groups: &BTreeMap<&str, &Group>,
    children: &BTreeSet<String>,
) -> Result<(), Error> {
    for unit in &graph.units {
        if let Some(parent) = &unit.parent {
            require(
                groups.contains_key(parent.as_str()) && children.contains(&unit.unit_id),
                "unit missing from parent",
            )?;
            if unit.kind == UnitKind::Code {
                require(
                    groups
                        .get(parent.as_str())
                        .is_some_and(|group| group.kind == GroupKind::Code),
                    "code unit must be a code group's child",
                )?;
            }
        } else {
            require(
                unit.kind != UnitKind::Code,
                "code unit requires a code group parent",
            )?;
        }
    }
    Ok(())
}

/// Rejects parent cycles while visiting each group's ancestry.
fn validate_acyclic(groups: &BTreeMap<&str, &Group>) -> Result<(), Error> {
    for id in groups.keys() {
        let mut path = BTreeSet::new();
        let mut cursor = Some(*id);
        while let Some(current) = cursor {
            require(path.insert(current), "ancestry cycle")?;
            cursor = groups
                .get(current)
                .and_then(|group| group.parent.as_deref());
        }
    }
    Ok(())
}

/// Returns ancestor context-part IDs in source order, excluding headings.
pub(super) fn required_context<'a>(
    graph: &'a DeliveryGraph,
    unit_id: &str,
) -> Result<Vec<&'a str>, Error> {
    let unit = graph
        .units
        .iter()
        .find(|unit| unit.unit_id == unit_id)
        .ok_or(Error::NotFound)?;
    let groups: BTreeMap<_, _> = graph
        .groups
        .iter()
        .map(|group| (group.group_id.as_str(), group))
        .collect();
    let mut context = BTreeSet::new();
    let mut parent = unit.parent.as_deref();
    while let Some(id) = parent {
        let group = groups
            .get(id)
            .ok_or(Error::Invalid("missing ancestor group"))?;
        context.extend(
            group
                .context_relations
                .iter()
                .map(|relation| relation.part_id.as_str()),
        );
        parent = group.parent.as_deref();
    }
    let parts: BTreeMap<_, _> = graph
        .parts
        .iter()
        .map(|part| (part.part_id.as_str(), part))
        .collect();
    let mut result: Vec<_> = context.into_iter().collect();
    result.sort_by_key(|part_id| {
        (
            parts
                .get(part_id)
                .and_then(|part| part.ranges.first())
                .map_or(u64::MAX, |range| range.start),
            *part_id,
        )
    });
    Ok(result)
}
