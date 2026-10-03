//! V2 policy links round-trip and policy identifiers retain their ASCII grammar.
use maestro_knowledge::collection::{Declaration, PolicyReference};
use serde_json::{Value, json};

/// Synthetic v2 declaration with a caller-selected policy link.
fn declaration(link: &Value) -> Value {
    json!({
        "schema": "maestro-collection/2", "id": "garden", "title": "Garden",
        "visibility": "public",
        "profiles": {"extraction": "html/1", "chunking": "chunks/1",
            "embedding": "embed:test", "sparse": "bm25/1"},
        "quality": {"ledger": "quality.jsonl"}, "sources": [],
        "evals": {"suite": "evals"}, "source_policy": link
    })
}

/// A policy reference with a synthetic immutable digest.
fn reference(id: &str) -> Value {
    json!({"id": id, "digest": "a".repeat(64)})
}

#[test]
fn v2_policy_links_round_trip_including_explicit_null() {
    for link in [Value::Null, reference("Policy_1.test-2")] {
        let wire = declaration(&link);
        let parsed: Declaration = wire.to_string().parse().unwrap();
        assert_eq!(serde_json::to_value(&parsed).unwrap(), wire);
        assert_eq!(
            parsed
                .source_policy
                .map(|policy| serde_json::to_value(policy).unwrap()),
            if link.is_null() { None } else { Some(link) }
        );
    }
}

#[test]
fn resource_ids_preserve_valid_ascii_at_length_boundary() {
    for id in ["A0-_z.9".to_owned(), "a".repeat(128)] {
        let policy: PolicyReference = serde_json::from_value(reference(&id)).unwrap();
        assert_eq!(policy.id, id);
    }
}

#[test]
fn resource_ids_refuse_each_invalid_component() {
    for id in [
        String::new(),
        "a".repeat(129),
        ".abc".to_owned(),
        "a/b".to_owned(),
        "aé".to_owned(),
    ] {
        let error = serde_json::from_value::<PolicyReference>(reference(&id)).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("invalid source-policy resource ID"),
            "{id:?}: {error}"
        );
    }
}
