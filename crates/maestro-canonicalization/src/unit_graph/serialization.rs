//! Stable graph payload serialization and digest.
use super::group_helpers::first_start;
use super::types::{DeliveryGraph, GraphDescriptor, Group, MappingArtifact, PartRole, SourcePart};
use crate::{error::Error, hashing::digest};
use std::{
    collections::{BTreeMap, BTreeSet},
    slice,
};

use super::wire_types::{
    WireContextRelation, WireExclusion, WireGraph, WireGraphDescriptor, WireGroup,
    WireMappingArtifact, WireMappingContribution, WireMappingEntry, WireMembership, WirePart,
    WireRetrievalView, WireSourceRange, WireUnit,
};

/// Compute the graph's content-addressed-storage digest over its exact wire bytes.
pub(super) fn graph_digest(graph: &DeliveryGraph) -> Result<String, Error> {
    Ok(digest(&serialize_graph(graph)?))
}

/// Serialize a graph with deterministic field and vector order.
///
/// # Errors
/// Returns an error if a graph record cannot be serialized.
pub fn serialize_graph(graph: &DeliveryGraph) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(&wire_graph(graph)).map_err(|error| Error(error.to_string()))
}

/// Parse exact graph wire bytes without reconstructing producer-only metadata.
#[cfg(test)]
pub(super) fn parse_wire_graph(bytes: &[u8]) -> Result<WireGraph, Error> {
    serde_json::from_slice(bytes).map_err(|error| Error(error.to_string()))
}

/// Parse the strict private mapping DTO for round-trip conformance tests.
#[cfg(test)]
pub(super) fn parse_wire_mapping(bytes: &[u8]) -> Result<WireMappingArtifact, Error> {
    serde_json::from_slice(bytes).map_err(|error| Error(error.to_string()))
}

/// Serialize a parsed wire DTO for canonical round-trip checks.
#[cfg(test)]
pub(super) fn serialize_wire_graph(graph: &WireGraph) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(graph).map_err(|error| Error(error.to_string()))
}

/// Serialize the strict private mapping DTO for round-trip conformance tests.
#[cfg(test)]
pub(super) fn serialize_wire_mapping(mapping: &WireMappingArtifact) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(mapping).map_err(|error| Error(error.to_string()))
}

/// Serialize the separate versioned canonical mapping CAS artifact.
///
/// # Errors
/// Returns an error if a mapping record cannot be serialized.
pub fn serialize_mapping(mapping: &MappingArtifact) -> Result<Vec<u8>, Error> {
    let wire = WireMappingArtifact {
        schema_version: mapping.schema_version.clone(),
        original_markdown_digest: mapping.original_markdown_digest.clone(),
        contributions: mapping
            .contributions
            .iter()
            .map(|entry| WireMappingEntry {
                range: entry.range.into(),
                mapping: (&entry.mapping).into(),
            })
            .collect(),
        exclusions: mapping.exclusions.iter().map(WireExclusion::from).collect(),
    };
    serde_json::to_vec(&wire).map_err(|error| Error(error.to_string()))
}

/// Compute the mapping artifact's lower-case SHA-256 digest over exact wire bytes.
pub(super) fn mapping_digest(mapping: &MappingArtifact) -> Result<String, Error> {
    Ok(digest(&serialize_mapping(mapping)?))
}

/// Copy producer metadata into the strict wire descriptor, omitting graph-only fields.
fn wire_descriptor(descriptor: &GraphDescriptor) -> WireGraphDescriptor {
    WireGraphDescriptor {
        schema_version: descriptor.schema_version.clone(),
        collection_id: descriptor.collection_id.clone(),
        source_namespace: descriptor.source_namespace.clone(),
        document_id: descriptor.document_id.clone(),
        revision_id: descriptor.revision_id.clone(),
        original_markdown_digest: descriptor.original_markdown_digest.clone(),
        profile_name: descriptor.profile_name.clone(),
        profile_digest: descriptor.profile_digest.clone(),
        preparation_name: descriptor.preparation_name.clone(),
        preparation_digest: descriptor.preparation_digest.clone(),
        counter_contract: descriptor.counter_contract.clone(),
        mapping_digest: descriptor.mapping_digest.clone(),
    }
}

/// Build the contract DTO, sorting unique parts by their first original-source range.
fn wire_graph(graph: &DeliveryGraph) -> WireGraph {
    let mut parts = BTreeMap::<&str, &SourcePart>::new();
    for part in graph
        .units
        .iter()
        .flat_map(|unit| &unit.parts)
        .chain(graph.groups.iter().flat_map(|group| &group.parts))
    {
        parts.entry(&part.part_id).or_insert(part);
    }
    let mut parts: Vec<_> = parts.into_iter().collect();
    parts.sort_by_key(|(part_id, part)| (first_start(slice::from_ref(part)), *part_id));
    let parts = parts
        .into_iter()
        .map(|(_, part)| WirePart {
            part_id: part.part_id.clone(),
            ranges: part
                .ranges
                .iter()
                .copied()
                .map(WireSourceRange::from)
                .collect(),
            mappings: part
                .mappings
                .iter()
                .map(WireMappingContribution::from)
                .collect(),
        })
        .collect();
    let units = graph
        .units
        .iter()
        .map(|unit| WireUnit {
            unit_id: unit.unit_id.clone(),
            kind: unit.kind.into(),
            parent: unit.parent_id.clone(),
            part_ids: unit
                .parts
                .iter()
                .filter(|part| part.role == PartRole::Primary)
                .map(|part| part.part_id.clone())
                .collect(),
            split: (&unit.split).into(),
        })
        .collect();
    let retrieval_views = graph
        .retrieval_views
        .iter()
        .map(|view| WireRetrievalView {
            rank_policy: view.rank_policy.clone(),
            retrieval_view_id: view.retrieval_view_id.clone(),
            chunk_id: view.chunk_id.clone(),
            prepared_input_digest: view.prepared_input_digest.clone(),
            token_count: view.token_count,
            memberships: view
                .memberships
                .iter()
                .map(|membership| WireMembership {
                    unit_id: membership.unit_id.clone(),
                    primary_part_ids: membership.primary_part_ids.clone(),
                })
                .collect(),
        })
        .collect();
    let groups = graph
        .groups
        .iter()
        .map(|group| wire_group(group, graph))
        .collect();
    WireGraph {
        descriptor: wire_descriptor(&graph.descriptor),
        parts,
        units,
        exclusions: graph.exclusions.iter().map(WireExclusion::from).collect(),
        retrieval_views,
        groups,
    }
}

/// Convert a group and its external internal relations to their inline wire shape.
fn wire_group(group: &Group, graph: &DeliveryGraph) -> WireGroup {
    let mut relations: Vec<_> = graph
        .context_relations
        .iter()
        .filter(|relation| relation.group_id == group.group_id)
        .collect();
    relations.sort_by_key(|relation| relation.ordinal);
    let mut part_ids: Vec<_> = group.heading.iter().cloned().collect();
    let mut seen: BTreeSet<_> = part_ids.iter().cloned().collect();
    part_ids.extend(
        relations
            .iter()
            .filter(|relation| seen.insert(relation.part_id.clone()))
            .map(|relation| relation.part_id.clone()),
    );
    WireGroup {
        group_id: group.group_id.clone(),
        kind: group.kind.into(),
        parent: group.parent_id.clone(),
        children: group.children.clone(),
        part_ids,
        heading: group.heading.clone(),
        context_relations: relations
            .into_iter()
            .map(|relation| WireContextRelation {
                kind: relation.kind.into(),
                part_id: relation.part_id.clone(),
            })
            .collect(),
        family: (&group.family).into(),
    }
}
