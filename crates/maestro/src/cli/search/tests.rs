use super::search_document;
use crate::cli::output::Output;
use crate::knowledge::RESPONSE_LIMIT_BYTES;
use maestro_kernel::evidence::Bundle;
use serde_json::{Value, json};
use std::process::ExitCode;

#[test]
fn deadline_error_uses_a_nonzero_exit_code() {
    let code = super::presentation::deadline_error(Output::new(true)).expect("deadline output");
    assert_eq!(code, ExitCode::from(1));
}

#[test]
fn cli_text_does_not_report_evidence_omission_when_only_inventory_was_removed() {
    let sets = (0..740)
        .map(|number| {
            json!({
                "value": format!("{number:04}-{}", "x".repeat(12)),
                "documents": 1,
            })
        })
        .collect::<Vec<_>>();
    let bundle: Bundle = serde_json::from_value(json!({
        "schema": "maestro-evidence/1",
        "collection": "collection",
        "generation": 1,
        "query": "question",
        "lang": "en",
        "routes": {
            "structured": "ok",
            "dense": {"unavailable": "x".repeat(2_000)},
        },
        "passages": [],
        "conflicts": [],
        "known_gaps": [],
        "budget": {"evidence_tokens": 0, "limit": 12_000},
        "trace": [],
        "inventory": {
            "kind": "documents_by_set",
            "set_filter": null,
            "total_documents": sets.len(),
            "sets": sets,
        }
    }))
    .expect("valid bundle with bounded inventory");

    let envelope = search_document(&bundle).expect("inventory omission fits");
    assert_eq!(envelope.omitted, ["inventory_items"]);
    assert!(envelope.truncated);
    let data = envelope.data.as_ref().expect("reduced bundle");
    assert_eq!(data["inventory"]["total_documents"], 740);
    assert!(data["inventory"]["sets"].as_array().unwrap().len() < 740);
    assert!(envelope.dropped.unwrap().inventory_items > 0);
    let text = super::presentation::search_text(&bundle, &envelope);
    assert!(text.contains("Search inventory truncated:"));
    assert!(!text.contains("lowest-ranked passages were omitted"));
}

#[test]
fn cli_search_omits_all_evidence_semantics_before_writing_oversized_json() {
    let bundle: Bundle = serde_json::from_value(json!({
        "schema": "maestro-evidence/1",
        "collection": "collection",
        "generation": 1,
        "query": "question",
        "lang": "en",
        "routes": {},
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
        "conflicts": [],
        "known_gaps": [],
        "budget": {"evidence_tokens": 0, "limit": 12_000},
        "trace": [{
            "n": 1,
            "routes": ["lexical"],
            "procedural": false,
            "chunk_ids": ["chunk"],
        }]
    }))
    .expect("valid evidence bundle");

    let envelope = search_document(&bundle).expect("bounded CLI response");
    assert!(envelope.truncated);
    assert_eq!(envelope.omitted, ["passages"]);
    assert_eq!(envelope.dropped.unwrap().passages, 1);
    let data: Value = envelope.data.expect("successful truncated data");
    assert_eq!(data["passages"], json!([]));
    assert_eq!(data["conflicts"], json!([]));
    assert_eq!(data["trace"], json!([]));
    assert_eq!(data["budget"]["evidence_tokens"], 0);
    assert!(envelope.error.is_none());
}
