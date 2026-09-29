//! Exact ownership and source accounting, with structural references by ID.
use super::{
    error::{Error, require},
    mapping::MappingLedger,
    types::{DeliveryGraph, Part, SourceRange},
};
use std::collections::{BTreeMap, BTreeSet};

/// Checks original Markdown coordinates, never derived-text coordinates.
fn span(range: SourceRange, source: &str) -> Result<(), Error> {
    let start = usize::try_from(range.start).map_err(|_| Error::Invalid("span overflow"))?;
    let end = usize::try_from(range.end).map_err(|_| Error::Invalid("span overflow"))?;
    require(
        start < end
            && end <= source.len()
            && source.is_char_boundary(start)
            && source.is_char_boundary(end),
        "invalid original UTF-8 span",
    )
}

/// Verifies part references, unit ownership, mapping, and source tiling.
pub(super) fn validate(
    graph: &DeliveryGraph,
    ledger: &MappingLedger,
    source: &str,
) -> Result<(), Error> {
    let parts = collect_parts(graph, source)?;
    primary_owners(graph, &parts)?;
    validate_mappings(graph, ledger)?;
    validate_partition(graph, ledger, source, &parts)
}

/// Validates part IDs, mappings and canonical source order.
fn collect_parts<'a>(
    graph: &'a DeliveryGraph,
    source: &str,
) -> Result<BTreeMap<&'a str, &'a Part>, Error> {
    let mut parts = BTreeMap::new();
    for part in &graph.parts {
        require(
            !part.part_id.trim().is_empty()
                && !part.ranges.is_empty()
                && part.ranges.len() == part.mappings.len()
                && parts.insert(part.part_id.as_str(), part).is_none(),
            "duplicate or invalid part",
        )?;
        let mut previous = 0;
        for (range, mapping) in part.ranges.iter().zip(&part.mappings) {
            span(*range, source)?;
            require(
                range.start >= previous,
                "unordered or overlapping part ranges",
            )?;
            previous = range.end;
            require(
                !mapping.unit_id.trim().is_empty()
                    && !mapping.mapping_mode.trim().is_empty()
                    && mapping.derived_range.0 < mapping.derived_range.1,
                "invalid canonical contribution",
            )?;
        }
    }
    require(
        graph.parts.windows(2).all(|pair| match pair {
            [first, second] => first.ranges.first().zip(second.ranges.first()).is_some_and(
                |(first_range, second_range)| {
                    (first_range.start, first.part_id.as_str())
                        <= (second_range.start, second.part_id.as_str())
                },
            ),
            _ => true,
        }),
        "parts are not in canonical source order",
    )?;
    Ok(parts)
}

/// Checks unique unit owners, group-only headings, and referenced context parts.
fn primary_owners<'a>(
    graph: &'a DeliveryGraph,
    parts: &BTreeMap<&'a str, &Part>,
) -> Result<(), Error> {
    let mut owners = BTreeSet::new();
    for unit in &graph.units {
        for part_id in &unit.part_ids {
            require(
                parts.contains_key(part_id.as_str()),
                "unit references unknown part",
            )?;
            require(
                owners.insert(part_id.as_str()),
                "part has multiple primary owners",
            )?;
        }
    }
    let headings: BTreeSet<_> = graph
        .groups
        .iter()
        .filter_map(|group| group.heading.as_deref())
        .collect();
    let context_parts: BTreeSet<_> = graph
        .groups
        .iter()
        .flat_map(|group| group.context_relations.iter())
        .map(|relation| relation.part_id.as_str())
        .collect();
    require(
        headings
            .iter()
            .all(|heading| parts.contains_key(heading) && !owners.contains(heading)),
        "heading part cannot have a primary unit owner",
    )?;
    require(
        parts.keys().all(|part| {
            owners.contains(part) || headings.contains(part) || context_parts.contains(part)
        }),
        "part has no unit, heading or context owner",
    )?;
    for group in &graph.groups {
        require(
            group
                .part_ids
                .iter()
                .all(|id| parts.contains_key(id.as_str())),
            "group references unknown part",
        )?;
    }
    for view in &graph.retrieval_views {
        for membership in &view.memberships {
            require(
                membership
                    .primary_part_ids
                    .iter()
                    .all(|id| parts.contains_key(id.as_str())),
                "membership references unknown part",
            )?;
        }
    }
    Ok(())
}

/// Checks exact one-to-one graph-part range and mapping ledger equality.
fn validate_mappings(graph: &DeliveryGraph, ledger: &MappingLedger) -> Result<(), Error> {
    let mut owned = BTreeMap::new();
    for part in &graph.parts {
        for (range, mapping) in part.ranges.iter().zip(&part.mappings) {
            require(
                owned.insert(*range, mapping).is_none(),
                "duplicate primary range",
            )?;
        }
    }
    require(
        ledger.contributions.len() == owned.len(),
        "missing mapping contribution",
    )?;
    for (entry, (range, mapping)) in ledger.contributions.iter().zip(&owned) {
        require(
            entry.range == *range && entry.mapping == **mapping,
            "mapping differs from ownership",
        )?;
    }
    Ok(())
}

/// Checks that all part ranges plus ineligible exclusions tile the original.
fn validate_partition(
    graph: &DeliveryGraph,
    ledger: &MappingLedger,
    source: &str,
    parts: &BTreeMap<&str, &Part>,
) -> Result<(), Error> {
    let mut ranges: Vec<_> = parts
        .values()
        .flat_map(|part| part.ranges.iter().copied())
        .collect();
    require(
        graph.exclusions == ledger.exclusions,
        "exclusion ledger differs",
    )?;
    let mut previous = 0;
    for exclusion in &graph.exclusions {
        span(exclusion.range, source)?;
        require(
            !exclusion.reason.trim().is_empty() && exclusion.range.start >= previous,
            "invalid exclusion",
        )?;
        previous = exclusion.range.end;
        ranges.push(exclusion.range);
    }
    ranges.sort_unstable();
    let mut end = 0;
    for range in ranges {
        require(range.start == end, "overlap or unaccounted source bytes")?;
        end = range.end;
    }
    require(end == source.len() as u64, "unaccounted source tail")
}
