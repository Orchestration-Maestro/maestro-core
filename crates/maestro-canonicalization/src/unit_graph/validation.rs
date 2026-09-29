//! Reject malformed graph identities, source ranges and exact memberships.
use super::{
    group_validation::validate_groups,
    retrieval_validation::{is_digest, valid_range, validate_views},
    serialization::graph_digest,
    types::{DeliveryGraph, MappingArtifact, PartRole, SourcePart, SourceRange},
};
use crate::error::Error;
use std::collections::{BTreeMap, BTreeSet};

/// Validate all graph records against the original UTF-8 Markdown.
pub(super) fn validate_graph(
    graph: &DeliveryGraph,
    mapping: &MappingArtifact,
    markdown: &str,
) -> Result<(), Error> {
    validate_descriptor(graph, mapping)?;
    let parts = validate_units(graph, markdown)?;
    validate_mapping(graph, mapping, &parts, markdown)?;
    validate_views(graph, markdown)?;
    validate_groups(graph, &parts)?;
    Ok(())
}

/// Check required graph identity and its canonical payload digest.
fn validate_descriptor(graph: &DeliveryGraph, mapping: &MappingArtifact) -> Result<(), Error> {
    let descriptor = &graph.descriptor;
    if descriptor.schema_version != "maestro-unit-graph/1"
        || descriptor.collection_id.trim().is_empty()
        || descriptor.source_namespace.trim().is_empty()
        || descriptor.document_id.trim().is_empty()
        || descriptor.revision_id.trim().is_empty()
        || !is_digest(&descriptor.original_markdown_digest)
        || !is_digest(&descriptor.profile_digest)
        || !is_digest(&descriptor.preparation_digest)
        || !is_digest(&descriptor.mapping_digest)
        || descriptor.counter_contract.trim().is_empty()
    {
        return Err(Error("unit graph descriptor is incomplete".into()));
    }
    if mapping.schema_version != "maestro-unit-mapping/1"
        || mapping.original_markdown_digest != descriptor.original_markdown_digest
    {
        return Err(Error(
            "mapping artifact identity differs from graph descriptor".into(),
        ));
    }
    let expected = graph_digest(graph)?;
    if expected != descriptor.graph_digest {
        return Err(Error("unit graph payload digest mismatch".into()));
    }
    Ok(())
}

/// Collect validated primary source parts and exact coverage ledger ownership.
fn validate_units(
    graph: &DeliveryGraph,
    markdown: &str,
) -> Result<BTreeMap<String, SourcePart>, Error> {
    let mut unit_ids = BTreeSet::new();
    let mut ownership = PartOwnership::default();
    for unit in &graph.units {
        if unit.unit_id.trim().is_empty() || !unit_ids.insert(unit.unit_id.as_str()) {
            return Err(Error("duplicate or empty delivery unit ID".into()));
        }
        if unit.parts.is_empty() {
            return Err(Error("delivery unit has no source parts".into()));
        }
        for (ordinal, part) in unit.parts.iter().enumerate() {
            validate_source_part(part, ordinal, markdown, &mut ownership)?;
        }
    }
    if !ownership
        .context_part_ids
        .is_subset(&ownership.primary_part_ids)
    {
        return Err(Error(
            "required context does not reference owned primary parts".into(),
        ));
    }
    validate_coverage(graph, markdown, ownership.owned_ranges)?;
    Ok(ownership.canonical_parts)
}

/// Accumulated source-part ownership checked while building the canonical part map.
#[derive(Default)]
struct PartOwnership {
    /// Unique primary part identifiers.
    primary_part_ids: BTreeSet<String>,
    /// Context identifiers that must refer to primary ownership.
    context_part_ids: BTreeSet<String>,
    /// Canonical source mapping for every observed part ID.
    canonical_parts: BTreeMap<String, SourcePart>,
    /// All ranges claiming primary ownership.
    owned_ranges: Vec<SourceRange>,
}

/// Validate one part's source mappings, identities and ownership.
fn validate_source_part(
    part: &SourcePart,
    ordinal: usize,
    markdown: &str,
    ownership: &mut PartOwnership,
) -> Result<(), Error> {
    if part.ordinal != ordinal || part.part_id.trim().is_empty() {
        return Err(Error("empty part ID or noncontiguous part ordinal".into()));
    }
    if part.ranges.len() != part.mappings.len()
        || part
            .ranges
            .windows(2)
            .filter_map(|pair| pair.first().zip(pair.get(1)))
            .any(|(left, right)| left.end > right.start)
        || part.mappings.iter().any(|mapping| {
            mapping.unit_id.trim().is_empty()
                || mapping.mapping_mode.trim().is_empty()
                || mapping.derived_range.0 >= mapping.derived_range.1
        })
    {
        return Err(Error(
            "source part ranges and mappings are not paired".into(),
        ));
    }
    match ownership.canonical_parts.get(&part.part_id) {
        Some(previous) if previous.ranges != part.ranges || previous.mappings != part.mappings => {
            return Err(Error("one part ID names different source mappings".into()));
        }
        Some(previous) if previous.role != PartRole::Primary && part.role == PartRole::Primary => {
            ownership
                .canonical_parts
                .insert(part.part_id.clone(), part.clone());
        }
        None => {
            ownership
                .canonical_parts
                .insert(part.part_id.clone(), part.clone());
        }
        Some(_) => {}
    }
    match part.role {
        PartRole::Primary if !ownership.primary_part_ids.insert(part.part_id.clone()) => {
            return Err(Error("primary part ID is duplicated".into()));
        }
        PartRole::RequiredContext => {
            ownership.context_part_ids.insert(part.part_id.clone());
        }
        PartRole::Primary => {}
    }
    if part.ranges.is_empty() {
        return Err(Error("source part has no byte ranges".into()));
    }
    if part
        .ranges
        .iter()
        .any(|range| !valid_range(range.start, range.end, markdown))
    {
        return Err(Error("invalid original UTF-8 source range".into()));
    }
    if part.role == PartRole::Primary {
        ownership.owned_ranges.extend(part.ranges.iter().copied());
    }
    Ok(())
}

/// Match ownership to coverage and exclusion ranges in canonical source order.
fn validate_coverage(
    graph: &DeliveryGraph,
    markdown: &str,
    mut owned_ranges: Vec<SourceRange>,
) -> Result<(), Error> {
    let mut expected: Vec<_> = graph
        .units
        .iter()
        .flat_map(|unit| {
            unit.parts
                .iter()
                .filter(|part| part.role == PartRole::Primary)
                .flat_map(move |part| {
                    part.ranges.iter().map(move |range| {
                        (
                            range.start,
                            range.end,
                            unit.unit_id.as_str(),
                            part.part_id.as_str(),
                        )
                    })
                })
        })
        .collect();
    expected.sort_unstable();
    let mut actual: Vec<_> = graph
        .coverage
        .iter()
        .map(|entry| {
            (
                entry.range.start,
                entry.range.end,
                entry.unit_id.as_str(),
                entry.part_id.as_str(),
            )
        })
        .collect();
    actual.sort_unstable();
    if expected != actual {
        return Err(Error(
            "source coverage ledger differs from primary part ownership".into(),
        ));
    }
    if graph.exclusions.iter().any(|exclusion| {
        !valid_range(exclusion.range.start, exclusion.range.end, markdown)
            || exclusion.reason.trim().is_empty()
    }) {
        return Err(Error(
            "source exclusion has an invalid range or reason".into(),
        ));
    }
    owned_ranges.sort_unstable();
    if owned_ranges
        .windows(2)
        .filter_map(|pair| pair.first().zip(pair.get(1)))
        .any(|(left, right)| left.end > right.start)
    {
        return Err(Error(
            "primary source mapping has duplicate ownership".into(),
        ));
    }
    if graph
        .coverage
        .windows(2)
        .filter_map(|pair| pair.first().zip(pair.get(1)))
        .any(|(left, right)| left.range > right.range)
        || graph
            .exclusions
            .windows(2)
            .filter_map(|pair| pair.first().zip(pair.get(1)))
            .any(|(left, right)| left.range > right.range)
    {
        return Err(Error("source ledger is not in source order".into()));
    }
    Ok(())
}

/// Check the separate mapping CAS artifact and exact eligible/excluded byte partition.
fn validate_mapping(
    graph: &DeliveryGraph,
    mapping: &MappingArtifact,
    parts: &BTreeMap<String, SourcePart>,
    markdown: &str,
) -> Result<(), Error> {
    use crate::hashing::digest;
    let mut expected = Vec::new();
    for part in parts.values().filter(|part| part.role == PartRole::Primary) {
        expected.extend(
            part.ranges
                .iter()
                .copied()
                .zip(part.mappings.iter().cloned()),
        );
    }
    expected.sort_by_key(|(range, _)| (range.start, range.end));
    let actual: Vec<_> = mapping
        .contributions
        .iter()
        .map(|entry| (entry.range, entry.mapping.clone()))
        .collect();
    if actual != expected {
        let mismatch = actual
            .iter()
            .zip(&expected)
            .position(|(left, right)| left != right);
        return Err(Error(format!(
            concat!(
                "mapping artifact contributions differ from primary ownership: ",
                "actual {}, expected {}, mismatch {:?}, ranges {:?} vs {:?}"
            ),
            actual.len(),
            expected.len(),
            mismatch,
            actual.iter().map(|(range, _)| *range).collect::<Vec<_>>(),
            expected.iter().map(|(range, _)| *range).collect::<Vec<_>>()
        )));
    }
    if mapping.exclusions != graph.exclusions {
        return Err(Error(
            "mapping artifact exclusions differ from graph".into(),
        ));
    }
    if digest(markdown.as_bytes()) != graph.descriptor.original_markdown_digest {
        return Err(Error(
            "original source digest differs from graph descriptor".into(),
        ));
    }
    if digest(&super::serialization::serialize_mapping(mapping)?) != graph.descriptor.mapping_digest
    {
        return Err(Error(
            "mapping artifact digest differs from graph descriptor".into(),
        ));
    }
    let mut ranges: Vec<_> = graph
        .coverage
        .iter()
        .map(|entry| entry.range)
        .chain(graph.exclusions.iter().map(|entry| entry.range))
        .collect();
    ranges.sort_unstable();
    let mut cursor = 0;
    for range in ranges {
        if range.start != cursor {
            return Err(Error(
                "eligible source ranges and exclusions do not partition the source".into(),
            ));
        }
        cursor = range.end;
    }
    if cursor != markdown.len() {
        return Err(Error(
            "eligible source ranges and exclusions do not cover the source".into(),
        ));
    }
    Ok(())
}
