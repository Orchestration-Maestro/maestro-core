//! Stable graph payload serialization and digest.
use super::types::{DeliveryGraph, MappingArtifact};
use crate::{error::Error, hashing::digest};
/// Compute the graph's content-addressed-storage digest over its exact wire bytes.
pub(super) fn graph_digest(graph: &DeliveryGraph) -> Result<String, Error> {
    let bytes = serialize_graph(graph)?;
    Ok(digest(&bytes))
}

/// Serialize a graph with deterministic field and vector order.
///
/// # Errors
/// Returns an error if a graph record cannot be serialized.
pub fn serialize_graph(graph: &DeliveryGraph) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(graph).map_err(|error| Error(error.to_string()))
}

/// Serialize the separate versioned canonical mapping CAS artifact.
///
/// # Errors
/// Returns an error if a mapping record cannot be serialized.
pub fn serialize_mapping(mapping: &MappingArtifact) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(mapping).map_err(|error| Error(error.to_string()))
}

/// Compute the mapping artifact's lower-case SHA-256 digest over exact wire bytes.
pub(super) fn mapping_digest(mapping: &MappingArtifact) -> Result<String, Error> {
    Ok(digest(&serialize_mapping(mapping)?))
}
