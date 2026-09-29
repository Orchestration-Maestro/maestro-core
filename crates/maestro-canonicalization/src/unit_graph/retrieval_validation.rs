//! Validate retrieval-view memberships against unit ancestry and source ranges.
use super::types::{DeliveryGraph, DeliveryUnit, PartRole, RetrievalMembership, SourcePart};
use crate::error::Error;
use std::collections::BTreeSet;

/// Check each retrieval view's identity, membership links and searchable coverage.
pub(super) fn validate_views(graph: &DeliveryGraph, markdown: &str) -> Result<(), Error> {
    let unit_ids: BTreeSet<_> = graph
        .units
        .iter()
        .map(|unit| unit.unit_id.as_str())
        .collect();
    let part_ids: BTreeSet<_> = graph
        .units
        .iter()
        .flat_map(|unit| unit.parts.iter().map(|part| part.part_id.as_str()))
        .collect();
    for view in &graph.retrieval_views {
        if view.retrieval_view_id.trim().is_empty()
            || view.chunk_id.trim().is_empty()
            || !is_digest(&view.prepared_input_digest)
            || !matches!(view.rank_policy.as_str(), "complete_ideas" | "v2_unit")
            || view.token_count == 0
        {
            return Err(Error("retrieval view identity or count is invalid".into()));
        }
        for membership in &view.memberships {
            validate_membership(graph, membership, &unit_ids, &part_ids, markdown)?;
        }
    }
    let searchable_parts: BTreeSet<_> = graph
        .retrieval_views
        .iter()
        .flat_map(|view| &view.memberships)
        .flat_map(|membership| &membership.primary_part_ids)
        .map(String::as_str)
        .collect();
    if graph
        .coverage
        .iter()
        .any(|entry| !searchable_parts.contains(entry.part_id.as_str()))
    {
        return Err(Error(
            "eligible primary part has no retrieval representation".into(),
        ));
    }
    Ok(())
}

/// Validate membership identifiers, ordered ownership and exact mapped ranges.
fn validate_membership(
    graph: &DeliveryGraph,
    membership: &RetrievalMembership,
    unit_ids: &BTreeSet<&str>,
    part_ids: &BTreeSet<&str>,
    markdown: &str,
) -> Result<(), Error> {
    if !unit_ids.contains(membership.unit_id.as_str())
        || membership.primary_part_ids.is_empty()
        || membership.primary_ranges.is_empty()
    {
        return Err(Error(
            "retrieval view has incomplete delivery membership".into(),
        ));
    }
    let unit = graph
        .units
        .iter()
        .find(|unit| unit.unit_id == membership.unit_id)
        .ok_or_else(|| Error("retrieval view names an unknown unit".into()))?;
    let context_parts = super::prepared::unit_context_parts(
        unit,
        &graph.units,
        &graph.groups,
        &graph.context_relations,
    )?;
    validate_membership_parts(membership, unit, &context_parts)?;
    let primary_ranges: Vec<_> = unit
        .parts
        .iter()
        .filter(|part| {
            part.role == PartRole::Primary && membership.primary_part_ids.contains(&part.part_id)
        })
        .flat_map(|part| part.ranges.iter().copied())
        .collect();
    let context_ranges: Vec<_> = context_parts
        .iter()
        .filter(|part| membership.context_part_ids.contains(&part.part_id))
        .flat_map(|part| part.ranges.iter().copied())
        .collect();
    if primary_ranges != membership.primary_ranges || context_ranges != membership.context_ranges {
        return Err(Error(format!(
            concat!(
                "retrieval membership ranges differ from named parts: ",
                "unit {}, primary {:?}/{:?}, context {:?}/{:?}"
            ),
            membership.unit_id,
            membership.primary_ranges,
            primary_ranges,
            membership.context_ranges,
            context_ranges
        )));
    }
    if membership
        .primary_part_ids
        .iter()
        .any(|id| !part_ids.contains(id.as_str()))
        || membership
            .context_part_ids
            .iter()
            .any(|id| !part_ids.contains(id.as_str()))
        || membership
            .primary_ranges
            .iter()
            .chain(&membership.context_ranges)
            .any(|range| !valid_range(range.start, range.end, markdown))
    {
        return Err(Error(
            "retrieval membership range or part link is invalid".into(),
        ));
    }
    Ok(())
}

/// Check membership part roles and source order against its unit and ancestors.
fn validate_membership_parts(
    membership: &RetrievalMembership,
    unit: &DeliveryUnit,
    context_parts: &[SourcePart],
) -> Result<(), Error> {
    let primary_ids: BTreeSet<_> = unit
        .parts
        .iter()
        .filter(|part| part.role == PartRole::Primary)
        .map(|part| part.part_id.as_str())
        .collect();
    let context_ids: BTreeSet<_> = context_parts
        .iter()
        .map(|part| part.part_id.as_str())
        .collect();
    if membership
        .primary_part_ids
        .iter()
        .any(|id| !primary_ids.contains(id.as_str()))
        || membership
            .context_part_ids
            .iter()
            .any(|id| !context_ids.contains(id.as_str()))
    {
        return Err(Error(
            "retrieval membership references a part with the wrong role".into(),
        ));
    }
    let expected_primary: Vec<_> = unit
        .parts
        .iter()
        .filter(|part| {
            part.role == PartRole::Primary && membership.primary_part_ids.contains(&part.part_id)
        })
        .map(|part| part.part_id.as_str())
        .collect();
    let expected_context: Vec<_> = context_parts
        .iter()
        .filter(|part| membership.context_part_ids.contains(&part.part_id))
        .map(|part| part.part_id.as_str())
        .collect();
    if expected_primary
        != membership
            .primary_part_ids
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
        || expected_context
            != membership
                .context_part_ids
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
    {
        return Err(Error(
            "retrieval membership part IDs are not in unit source order".into(),
        ));
    }
    Ok(())
}

/// Check a lower-case SHA-256 hexadecimal digest.
pub(super) fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Check a nonempty half-open range on UTF-8 boundaries.
pub(super) fn valid_range(start: usize, end: usize, markdown: &str) -> bool {
    start < end
        && end <= markdown.len()
        && markdown.is_char_boundary(start)
        && markdown.is_char_boundary(end)
}
