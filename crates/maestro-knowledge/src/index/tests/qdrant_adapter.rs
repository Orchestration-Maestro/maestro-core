//! Unit tests of the Qdrant adapter's hit conversion.

use crate::index::{
    CollectionLayout, ProjectionError, Qdrant, RetrievalProjectionPort, qdrant_adapter::point_hit,
};
use qdrant_client::qdrant::{Value, value::Kind};
use std::collections::HashMap;

#[tokio::test]
async fn adapter_rejects_invalid_layout_as_a_port_error_before_io() {
    let qdrant = Qdrant::new("http://127.0.0.1:1").unwrap();
    let error: ProjectionError = qdrant
        .create_collection(
            "never-created",
            CollectionLayout {
                dense_dimensions: 8,
                dense_present: true,
                dense_distance: "Euclid".to_owned(),
                sparse_present: true,
                sparse_modifier: Some("Idf".to_owned()),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "unsupported retrieval vector layout");
}

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
