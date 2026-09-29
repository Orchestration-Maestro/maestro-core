//! Retrieval membership identity, ordering and content coverage.
use super::{
    error::{Error, require},
    types::DeliveryGraph,
    types::SplitMarker,
};
use std::collections::{BTreeMap, BTreeSet};

/// Checks continuation identity, view membership and searchable-part coverage.
pub(super) fn validate(graph: &DeliveryGraph) -> Result<(), Error> {
    validate_units(graph)?;
    let covered = validate_views(graph)?;
    validate_searchability(graph, &covered)
}

/// Validates required unit IDs and continuation bounds.
fn validate_units(graph: &DeliveryGraph) -> Result<(), Error> {
    for unit in &graph.units {
        require(
            !unit.unit_id.trim().is_empty() && !unit.part_ids.is_empty(),
            "unit requires primary parts",
        )?;
        if let SplitMarker::Continuation { ordinal, total } = unit.split {
            require(total > 1 && ordinal < total, "invalid continuation")?;
        }
    }
    Ok(())
}

/// Validates views, membership subsets and their canonical order.
fn validate_views(graph: &DeliveryGraph) -> Result<BTreeSet<&str>, Error> {
    let parts: BTreeMap<_, _> = graph
        .parts
        .iter()
        .map(|part| (part.part_id.as_str(), part))
        .collect();
    let units: BTreeMap<_, _> = graph
        .units
        .iter()
        .map(|unit| (unit.unit_id.as_str(), unit))
        .collect();
    let unit_order: BTreeMap<_, _> = graph
        .units
        .iter()
        .enumerate()
        .map(|(index, unit)| (unit.unit_id.as_str(), index))
        .collect();
    let mut covered = BTreeSet::new();
    let mut view_ids = BTreeSet::new();
    let mut chunk_ids = BTreeSet::new();
    let mut view_keys = Vec::new();
    for view in &graph.retrieval_views {
        require(
            !view.chunk_id.trim().is_empty()
                && !view.retrieval_view_id.trim().is_empty()
                && view.token_count > 0
                && view_ids.insert(view.retrieval_view_id.as_str())
                && chunk_ids.insert(view.chunk_id.as_str()),
            "invalid or duplicate retrieval view",
        )?;
        validate_memberships(view, &units, &unit_order, &mut covered)?;
        let start = view
            .memberships
            .first()
            .and_then(|membership| membership.primary_part_ids.first())
            .and_then(|id| parts.get(id.as_str()))
            .and_then(|part| part.ranges.first())
            .ok_or(Error::Invalid("view without primary content"))?
            .start;
        view_keys.push((start, view.retrieval_view_id.as_str()));
    }
    require(
        view_keys.windows(2).all(|pair| match pair {
            [first, second] => first <= second,
            _ => true,
        }),
        "views are not in canonical source order",
    )?;
    Ok(covered)
}

/// Checks each member's ordered primary subset and accumulates covered parts.
fn validate_memberships<'a>(
    view: &'a super::types::RetrievalView,
    units: &BTreeMap<&str, &super::types::DeliveryUnit>,
    unit_order: &BTreeMap<&str, usize>,
    covered: &mut BTreeSet<&'a str>,
) -> Result<(), Error> {
    let mut previous_unit = None;
    for membership in &view.memberships {
        let unit = units
            .get(membership.unit_id.as_str())
            .ok_or(Error::Invalid("unknown membership unit"))?;
        require(
            !membership.primary_part_ids.is_empty(),
            "membership requires primary parts",
        )?;
        let mut last = None;
        for id in &membership.primary_part_ids {
            let index = unit
                .part_ids
                .iter()
                .position(|part| part == id)
                .ok_or(Error::Invalid(
                    "membership part is not primary unit content",
                ))?;
            require(
                last.is_none_or(|previous| previous < index),
                "membership parts are not in unit order",
            )?;
            last = Some(index);
            covered.insert(id.as_str());
        }
        let order = *unit_order
            .get(membership.unit_id.as_str())
            .ok_or(Error::Invalid("unknown membership unit"))?;
        require(
            previous_unit.is_none_or(|previous| previous < order),
            "memberships are not in unit order",
        )?;
        previous_unit = Some(order);
    }
    Ok(())
}

/// Requires each part in a membership, context relation or heading.
fn validate_searchability(graph: &DeliveryGraph, covered: &BTreeSet<&str>) -> Result<(), Error> {
    let context_or_heading: BTreeSet<_> = graph
        .groups
        .iter()
        .flat_map(|group| {
            group
                .context_relations
                .iter()
                .map(|relation| relation.part_id.as_str())
                .chain(group.heading.iter().map(String::as_str))
        })
        .collect();
    for unit in &graph.units {
        for part in &unit.part_ids {
            require(
                covered.contains(part.as_str()) || context_or_heading.contains(part.as_str()),
                "unsearchable primary part",
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Loads the fixture graph without running graph-level validation.
    fn graph() -> DeliveryGraph {
        DeliveryGraph::from_bytes(
            include_bytes!("../../tests/fixtures/unit-graph-v1.json")
                .strip_suffix(b"\n")
                .unwrap(),
        )
        .unwrap()
    }

    /// A delivery unit needs both a stable ID and primary source parts.
    #[test]
    fn unit_validation_rejects_an_empty_identifier() {
        let mut graph = graph();
        graph.units[0].unit_id.clear();
        assert!(validate_units(&graph).is_err());
    }

    /// Repeated membership parts violate their strictly increasing positions.
    #[test]
    fn membership_validation_rejects_a_repeated_primary_part() {
        let graph = graph();
        let mut view = graph.retrieval_views[0].clone();
        view.memberships[0].primary_part_ids[1] = "part-3".into();
        let units = graph
            .units
            .iter()
            .map(|unit| (unit.unit_id.as_str(), unit))
            .collect();
        let unit_order = graph
            .units
            .iter()
            .enumerate()
            .map(|(index, unit)| (unit.unit_id.as_str(), index))
            .collect();
        assert!(validate_memberships(&view, &units, &unit_order, &mut BTreeSet::new()).is_err());
    }
}
