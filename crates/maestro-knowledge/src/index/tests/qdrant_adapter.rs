//! Unit tests of the Qdrant adapter's hit conversion.

use crate::index::qdrant_adapter::point_hit;
use qdrant_client::qdrant::{Value, value::Kind};
use std::collections::HashMap;

#[test]
fn adapter_converts_payload_values_without_requiring_a_point_id() {
    let payload = HashMap::from([(
        "field".to_owned(),
        Value {
            kind: Some(Kind::StringValue("value".to_owned())),
        },
    )]);
    let hit = point_hit(None, payload, Some(0.5));
    assert!(hit.id.is_empty());
    assert_eq!(hit.score, Some(0.5));
    assert_eq!(hit.payload.get("field"), Some(&serde_json::json!("value")));
}
