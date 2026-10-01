//! Canonical graph array and node part ordering.
use super::{
    error::{Error, require},
    types::{DeliveryGraph, DeliveryUnit, Group, Part},
};
use std::collections::{BTreeMap, BTreeSet};

/// Checks unique node part references and source-derived array ordering.
pub(super) fn validate(graph: &DeliveryGraph) -> Result<(), Error> {
    let parts: BTreeMap<_, _> = graph
        .parts
        .iter()
        .map(|part| (part.part_id.as_str(), part))
        .collect();
    for ids in graph
        .units
        .iter()
        .map(|unit| &unit.part_ids)
        .chain(graph.groups.iter().map(|group| &group.part_ids))
    {
        let mut seen = BTreeSet::new();
        require(
            ids.iter().all(|id| seen.insert(id.as_str())),
            "duplicate part ID within node",
        )?;
        require(
            ids.windows(2).all(|pair| match pair {
                [first, second] => position(&parts, first) <= position(&parts, second),
                _ => true,
            }),
            "node parts are not in source order",
        )?;
    }
    let ordered = |items: Vec<(u64, &str)>| {
        items.windows(2).all(|pair| match pair {
            [first, second] => first <= second,
            _ => true,
        })
    };
    require(
        ordered(
            graph
                .units
                .iter()
                .map(|unit| (first_start(&unit.part_ids, &parts), unit.unit_id.as_str()))
                .collect(),
        ),
        "units are not in canonical source order",
    )?;
    let units: BTreeMap<_, _> = graph
        .units
        .iter()
        .map(|unit| (unit.unit_id.as_str(), unit))
        .collect();
    let groups: BTreeMap<_, _> = graph
        .groups
        .iter()
        .map(|group| (group.group_id.as_str(), group))
        .collect();
    require(
        ordered(
            graph
                .groups
                .iter()
                .map(|group| {
                    (
                        group_start(group.group_id.as_str(), &groups, &units, &parts),
                        group.group_id.as_str(),
                    )
                })
                .collect(),
        ),
        "groups are not in canonical source order",
    )?;
    Ok(())
}

/// Returns source position and stable ID for a part.
fn position<'a>(parts: &BTreeMap<&str, &Part>, id: &'a str) -> (u64, &'a str) {
    (
        parts
            .get(id)
            .and_then(|part| part.ranges.first())
            .map_or(u64::MAX, |range| range.start),
        id,
    )
}
/// Finds the first source contribution in a group's descendant tree.
fn group_start(
    id: &str,
    groups: &BTreeMap<&str, &Group>,
    units: &BTreeMap<&str, &DeliveryUnit>,
    parts: &BTreeMap<&str, &Part>,
) -> u64 {
    let Some(group) = groups.get(id) else {
        return u64::MAX;
    };
    group
        .part_ids
        .iter()
        .map(|part| position(parts, part).0)
        .chain(group.children.iter().map(|child| {
            units.get(child.as_str()).map_or_else(
                || group_start(child, groups, units, parts),
                |unit| first_start(&unit.part_ids, parts),
            )
        }))
        .min()
        .unwrap_or(u64::MAX)
}

/// Finds the earliest primary part used by a unit.
fn first_start(ids: &[String], parts: &BTreeMap<&str, &Part>) -> u64 {
    ids.iter()
        .map(|id| position(parts, id).0)
        .min()
        .unwrap_or(u64::MAX)
}
