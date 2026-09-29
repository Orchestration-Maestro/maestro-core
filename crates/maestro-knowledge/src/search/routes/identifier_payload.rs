//! Validation of identifier-route payload results.

use super::results::ScoredChunk;
use crate::{
    index::{PointHit, ProjectionError, invalid_answer as projection_invalid_answer, payload_text},
    query::PROFILE,
};

/// Validates the required string and string-array fields of a payload hit.
pub(super) fn payload_hit(point: &PointHit) -> Result<ScoredChunk, ProjectionError> {
    let chunk_id = required_text(point, "chunk_id")?;
    let revision_id = required_text(point, "revision_id")?;
    if required_text(point, "identifier_profile")? != PROFILE {
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
    projection_invalid_answer(reason)
}

/// Reads and validates one required string field of a Qdrant payload.
fn required_text(point: &PointHit, field: &str) -> Result<String, ProjectionError> {
    payload_text(point, field)
        .map(str::to_owned)
        .ok_or_else(|| invalid_answer(&format!("Qdrant hit lacks string {field}")))
}

#[cfg(test)]
mod tests {
    use super::payload_hit;
    use crate::index::PointHit;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn hit(profile: serde_json::Value, identifiers: serde_json::Value) -> PointHit {
        PointHit {
            id: "point".to_owned(),
            score: None,
            payload: BTreeMap::from([
                ("chunk_id".to_owned(), json!("chunk")),
                ("revision_id".to_owned(), json!("revision")),
                ("identifier_profile".to_owned(), profile),
                ("identifiers".to_owned(), identifiers),
            ]),
        }
    }

    #[test]
    fn validates_profile_and_string_identifier_values() {
        assert!(payload_hit(&hit(json!("wrong"), json!(["a"]))).is_err());
        assert!(payload_hit(&hit(json!("identifier-v1"), json!([1]))).is_err());
        assert!(payload_hit(&hit(json!("identifier-v1"), json!("not an array"))).is_err());
    }
}
