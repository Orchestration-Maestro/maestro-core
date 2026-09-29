use super::search_document;
use crate::cli::output::Output;
use crate::knowledge::{RESPONSE_LIMIT_BYTES, RefreshScratch};
use maestro_kernel::evidence::Bundle;
use maestro_knowledge::search::SourceClassTable;
use serde_json::{Value, json};
use std::{collections::BTreeMap, process::ExitCode};

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
        "budget": {"evidence_bytes": 0, "limit": 12_000},
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
    let text = super::presentation::search_text(&bundle, &envelope, &BTreeMap::new());
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
        "budget": {"evidence_bytes": 0, "limit": 12_000},
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
    assert_eq!(data["budget"]["evidence_bytes"], 0);
    assert!(envelope.error.is_none());
}

fn passage(n: u32, document: &str, version: Option<&str>, source_ref: &str) -> Value {
    json!({
        "n": n,
        "doc_id": document,
        "revision_id": format!("{document}-revision"),
        "title": format!("{document} title"),
        "section_path": if n == 3 { json!(["Install", "Agents"]) } else { json!([]) },
        "version": version,
        "source_ref": source_ref,
        "span": [0, 4],
        "digest": format!("sha256:{}", "a".repeat(64)),
        "text": format!("{document} text {n}"),
        "alternates": [],
    })
}

#[test]
fn cli_text_lists_each_document_once_at_its_best_rank_with_a_readable_label() {
    let mut passages = vec![
        passage(
            1,
            "install",
            Some("9.0.22"),
            "https://docs.example.org/install.htm",
        ),
        passage(2, "repo", None, "corpus-path:repos/tools/scale.md"),
        passage(
            3,
            "install",
            Some("9.0.22"),
            "https://docs.example.org/install.htm",
        ),
        passage(4, "other", None, "https://other.example.org/page"),
    ];
    for value in &mut passages {
        if value["version"].is_null() {
            value.as_object_mut().unwrap().remove("version");
        }
    }
    let bundle: Bundle = serde_json::from_value(json!({
        "schema": "maestro-evidence/1",
        "collection": "collection",
        "generation": 1,
        "query": "install at scale",
        "lang": "en",
        "routes": {},
        "passages": passages,
        "conflicts": [],
        "known_gaps": [],
        "budget": {"evidence_bytes": 0, "limit": 12_000},
        "trace": (1..=4)
            .map(|n| json!({"n": n, "routes": ["lexical"], "procedural": false}))
            .collect::<Vec<_>>(),
    }))
    .expect("valid evidence bundle");
    let envelope = search_document(&bundle).expect("bounded CLI response");
    let labels = BTreeMap::from([
        ("install".to_owned(), "Example docs · Product".to_owned()),
        ("repo".to_owned(), "Example repo".to_owned()),
    ]);
    let text = super::presentation::search_text(&bundle, &envelope, &labels);
    assert_eq!(
        text,
        "Search collection generation 1: install at scale\n\n\
         1. Example docs · Product 9.0.22 — install title\n\
         [1]\n\
         install text 1\n\
         [3] Install › Agents\n\
         install text 3\n\n\
         2. Example repo — repo title\n\
         [2]\n\
         repo text 2\n\n\
         3. other.example.org — other title\n\
         [4]\n\
         other text 4\n"
    );
    let unlabelled = super::presentation::search_text(&bundle, &envelope, &BTreeMap::new());
    assert!(unlabelled.contains("\n1. docs.example.org 9.0.22 — install title\n[1]\n"));
    assert!(unlabelled.contains("\n2. repos — repo title\n[2]\n"));
    let data = envelope.data.expect("every passage stays in JSON");
    assert_eq!(data["passages"].as_array().unwrap().len(), 4);
}

#[test]
fn text_labels_name_each_classified_passage_document() {
    let scratch = RefreshScratch::new();
    let kernel = scratch.kernel(None).unwrap();
    let table = SourceClassTable::parse(
        br#"{"schema": "maestro-source-classes/1", "rules": [
            {"path_prefix": "source:docs", "class": "official_docs", "label": "Docs"}
        ]}"#,
    )
    .unwrap();
    let mut known = passage(1, "document", None, "source:docs");
    known["revision_id"] = json!("revision");
    let bundle: Bundle = serde_json::from_value(json!({
        "schema": "maestro-evidence/1",
        "collection": "collection",
        "generation": 1,
        "query": "question",
        "lang": "en",
        "routes": {},
        "passages": [known, passage(2, "unknown", None, "source:docs")],
        "conflicts": [],
        "known_gaps": [],
        "budget": {"evidence_bytes": 0, "limit": 12_000},
        "trace": [],
    }))
    .unwrap();
    let labels =
        super::execution::passage_labels(Some(&table), &bundle, &kernel.database, &kernel.scopes);
    assert_eq!(
        labels,
        BTreeMap::from([("document".to_owned(), "Docs".to_owned())])
    );
    assert!(
        super::execution::passage_labels(None, &bundle, &kernel.database, &kernel.scopes)
            .is_empty()
    );
}
