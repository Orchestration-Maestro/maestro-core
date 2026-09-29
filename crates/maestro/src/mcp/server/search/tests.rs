use super::{super::response::response_fits, bounded_result};
use crate::knowledge::RESPONSE_LIMIT_BYTES;
use maestro_kernel::evidence::Bundle;
use rmcp::model::{ProtocolVersion, RequestId};
use serde_json::{Value, json};

const PROTOCOL: ProtocolVersion = ProtocolVersion::V_2025_11_25;

#[test]
fn search_drops_inventory_groups_with_counts_and_keeps_reduced_json_in_text() {
    let groups = (0..740)
        .map(|number| {
            json!({
                "value": format!("{number:04}-{}", "x".repeat(12)),
                "documents": 1,
            })
        })
        .collect::<Vec<_>>();
    let total_groups = groups.len();
    let bundle = bundle(&json!({
        "routes": {"structured": "ok"},
        "inventory": {
            "kind": "documents_by_set",
            "set_filter": null,
            "total_documents": groups.len(),
            "sets": groups,
        }
    }));
    let result = bounded_result(bundle, &RequestId::Number(1), Some(&PROTOCOL))
        .expect("bounded inventory omission");
    let value = serde_json::to_value(&result).expect("serialized result");
    let retained_groups = value["structuredContent"]["inventory"]["sets"]
        .as_array()
        .expect("reduced inventory groups");
    let dropped_groups = total_groups - retained_groups.len();

    assert_eq!(value["isError"], false);
    assert_eq!(
        value["structuredContent"]["inventory"]["total_documents"],
        total_groups
    );
    assert!(dropped_groups > 0);
    assert_eq!(
        value["_meta"]["maestro/truncation"]["dropped"]["inventory_items"],
        dropped_groups
    );
    let text_bundle: Value = serde_json::from_str(
        value["content"][0]["text"]
            .as_str()
            .expect("JSON content for text-only clients"),
    )
    .expect("parse reduced text bundle");
    assert_eq!(text_bundle, value["structuredContent"]);
    assert!(
        value["content"][1]["text"]
            .as_str()
            .expect("separate truncation warning")
            .contains("not exhaustive")
    );
    assert!(response_fits(&result, &RequestId::Number(1), Some(&PROTOCOL)).expect("wire fit"));
}

#[test]
fn search_drops_oversized_passages_and_keeps_reduced_json_in_text() {
    let bundle = bundle(&json!({
        "passages": [{
            "n": 1,
            "doc_id": "document",
            "revision_id": "revision",
            "title": "Source",
            "section_path": [],
            "source_ref": "source",
            "span": [0, RESPONSE_LIMIT_BYTES],
            "digest": format!("sha256:{}", "a".repeat(64)),
            "text": "x".repeat(RESPONSE_LIMIT_BYTES),
            "alternates": [],
        }],
        "trace": [{
            "n": 1,
            "routes": ["lexical"],
            "procedural": false,
            "chunk_ids": ["chunk"],
        }]
    }));
    let result = bounded_result(bundle, &RequestId::Number(1), Some(&PROTOCOL))
        .expect("bounded evidence omission");
    let value = serde_json::to_value(&result).expect("serialized result");

    assert_eq!(value["isError"], false);
    assert_eq!(value["structuredContent"]["passages"], json!([]));
    assert_eq!(value["structuredContent"]["conflicts"], json!([]));
    assert_eq!(value["structuredContent"]["trace"], json!([]));
    assert_eq!(value["structuredContent"]["budget"]["evidence_bytes"], 0);
    assert_eq!(
        value["_meta"]["maestro/truncation"]["omitted"],
        json!(["passages"])
    );
    assert_eq!(
        value["_meta"]["maestro/truncation"]["dropped"]["passages"],
        1
    );
    let text_bundle: Value = serde_json::from_str(
        value["content"][0]["text"]
            .as_str()
            .expect("JSON content for text-only clients"),
    )
    .expect("parse reduced text bundle");
    assert_eq!(text_bundle, value["structuredContent"]);
    assert!(
        value["content"][1]["text"]
            .as_str()
            .expect("separate truncation warning")
            .contains("not exhaustive")
    );
    assert!(response_fits(&result, &RequestId::Number(1), Some(&PROTOCOL)).expect("wire fit"));
}

#[test]
fn search_refuses_a_response_that_cannot_be_trimmed_without_losing_query_scope() {
    let bundle = bundle(&json!({
        "query": "q".repeat(RESPONSE_LIMIT_BYTES + 1),
    }));
    let result =
        bounded_result(bundle, &RequestId::Number(1), Some(&PROTOCOL)).expect("bounded refusal");
    let result: Value = serde_json::to_value(result).expect("serialized refusal");
    assert_eq!(result["isError"], true);
    let error: Value = serde_json::from_str(
        result["content"][0]["text"]
            .as_str()
            .expect("tool error content"),
    )
    .expect("typed error document");
    assert_eq!(error["error"]["code"], "response_too_large");
}

fn bundle(overrides: &Value) -> Bundle {
    let mut bundle = json!({
        "schema": "maestro-evidence/1",
        "collection": "collection",
        "generation": 1,
        "query": "question",
        "lang": "en",
        "routes": {},
        "passages": [],
        "conflicts": [],
        "known_gaps": [],
        "budget": {"evidence_bytes": 0, "limit": 12_000},
        "trace": []
    });
    for (key, value) in overrides.as_object().expect("bundle overrides") {
        bundle[key] = value.clone();
    }
    serde_json::from_value(bundle).expect("valid evidence bundle")
}
