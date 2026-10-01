//! Qdrant point responses filtered by requested payload and vectors.

use qdrant_client::qdrant::{RetrievedPoint, with_payload_selector, with_vectors_selector};
use std::collections::HashMap;

/// `point` with its payload when `payload` asks for it, and its vectors when
/// `vectors` does.
pub(super) fn shown(
    point: &RetrievedPoint,
    payload: Option<&with_payload_selector::SelectorOptions>,
    vectors: Option<&with_vectors_selector::SelectorOptions>,
) -> RetrievedPoint {
    let payload = matches!(
        payload,
        Some(with_payload_selector::SelectorOptions::Enable(true))
    );
    let vectors = matches!(
        vectors,
        Some(with_vectors_selector::SelectorOptions::Enable(true))
    );
    RetrievedPoint {
        id: point.id.clone(),
        payload: if payload {
            point.payload.clone()
        } else {
            HashMap::new()
        },
        vectors: if vectors { point.vectors.clone() } else { None },
        shard_key: None,
        order_value: None,
    }
}
