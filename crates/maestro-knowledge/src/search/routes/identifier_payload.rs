//! Validation of identifier-route payload results.

use super::results::ScoredChunk;
use crate::{
    index::{PointHit, ProjectionError},
    query::PROFILE,
};

/// Validates the required string and string-array fields of a payload hit.
pub(super) fn payload_hit(point: &PointHit) -> Result<ScoredChunk, ProjectionError> {
    let chunk_id = payload_text(point, "chunk_id")?;
    let revision_id = payload_text(point, "revision_id")?;
    if payload_text(point, "identifier_profile")? != PROFILE {
        return Err(invalid_answer(
            "Qdrant returned an invalid identifier profile",
        ));
    }
    let Some(values) = point
        .payload
        .get("identifiers")
        .and_then(serde_json::Value::as_array)
    else {
        return Err(invalid_answer(
            "Qdrant hit lacks a string-array identifiers field",
        ));
    };
    if values.iter().any(|value| !value.is_string()) {
        return Err(invalid_answer(
            "Qdrant hit has a malformed identifiers array",
        ));
    }
    Ok(ScoredChunk {
        chunk_id,
        revision_id,
        score: 1.0,
    })
}

/// Wraps malformed payload or scroll answers in a backend error.
pub(super) fn invalid_answer(reason: &str) -> ProjectionError {
    ProjectionError::new(format!(
        "Qdrant's answer is not what was asked for: {reason}"
    ))
}

/// Gets a string-valued field from a projection payload.
fn payload_text(point: &PointHit, field: &str) -> Result<String, ProjectionError> {
    point
        .payload
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid_answer(&format!("Qdrant hit lacks string {field}")))
}
