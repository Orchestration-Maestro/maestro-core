//! Verify the producer's separate mapping artifact and source partition.
use super::types::{DeliveryGraph, MappingArtifact, PartRole, SourcePart, SourceRange};
use crate::{error::Error, hashing::digest};
use std::collections::BTreeMap;

/// Check the producer mapping artifact's integrity without duplicating kernel graph validation.
pub(super) fn validate_graph(
    graph: &DeliveryGraph,
    mapping: &MappingArtifact,
    markdown: &str,
) -> Result<(), Error> {
    if mapping.schema_version != "maestro-unit-mapping/1"
        || mapping.original_markdown_digest != graph.descriptor.original_markdown_digest
    {
        return Err(Error(
            "mapping artifact identity differs from graph descriptor".into(),
        ));
    }
    let mut parts = BTreeMap::<&str, &SourcePart>::new();
    for part in graph
        .units
        .iter()
        .flat_map(|unit| &unit.parts)
        .filter(|part| part.role == PartRole::Primary)
        .chain(graph.groups.iter().flat_map(|group| &group.parts))
    {
        parts.insert(part.part_id.as_str(), part);
    }
    let mut expected: Vec<_> = parts
        .values()
        .flat_map(|part| {
            part.ranges
                .iter()
                .copied()
                .zip(part.mappings.iter().cloned())
        })
        .collect();
    expected.sort_by_key(|(range, _)| (range.start, range.end));
    let actual: Vec<_> = mapping
        .contributions
        .iter()
        .map(|entry| (entry.range, entry.mapping.clone()))
        .collect();
    if actual != expected {
        return Err(Error(
            "mapping artifact differs from graph part mappings".into(),
        ));
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
    let mut ranges: Vec<SourceRange> = mapping
        .contributions
        .iter()
        .map(|entry| entry.range)
        .chain(mapping.exclusions.iter().map(|entry| entry.range))
        .collect();
    ranges.sort_unstable();
    let mut cursor = 0;
    for range in ranges {
        if range.start != cursor || range.end <= range.start {
            return Err(Error(
                "source mapping and exclusions do not partition the source".into(),
            ));
        }
        cursor = range.end;
    }
    if cursor != markdown.len() {
        return Err(Error(
            "source mapping and exclusions do not cover the source".into(),
        ));
    }
    Ok(())
}
