//! Only post-context, post-wire evidence earns complete-proof credit.

use super::super::delivered::anchors;
use maestro_kernel::{artifact::Digest, evidence::Bundle};
use maestro_knowledge::eval::{Located, proof_complete};
use serde_json::json;
use std::slice;

/// An assembled bundle: a small first anchor and a costly second passage.
fn bundle() -> Bundle {
    let passages: Vec<_> = [(1,
        "first".to_owned()), (2,
        "é".repeat(3_000))].into_iter()
        .map(|(number, text)| json!({
        "n":number, "doc_id":format!("doc-{number}"), "revision_id":format!("revision-{number}"),
        "title":"Guide", "section_path":[], "source_ref":"corpus-path:guide.md",
        "span":[0,text.len()], "digest":format!("sha256:{}",Digest::of(text.as_bytes()).as_str()),
        "text":text, "alternates":[]
    })).collect();
    serde_json::from_value(json!({
        "schema":"maestro-evidence/1", "collection":"synthetic", "generation":1,
        "query":"private-question", "lang":"en", "routes":{"structured":"ok"},
        "passages":passages,"conflicts":[],"known_gaps":[],
        "budget":{"evidence_bytes":7000,
        "limit":12000,
        "counter":"evidence-utf8-bytes/1",
        "estimated":true},
        "trace":[], "inventory":{"kind":"documents_by_set","set_filter":null,"total_documents":450,
            "sets":(0..450).map(|number|json!({"value":format!("{number:04}-{}",
        "x".repeat(26)),
        "documents":1})).collect::<Vec<_>>()}
    }))
    .unwrap()
}

#[test]
fn graph_eval_delivered_adapter_does_not_credit_wire_dropped_supports() {
    let actual = anchors(bundle()).unwrap();
    let first = Located {
        revision_id: "revision-1".to_owned(),
        span: [0, 5],
    };
    let second = Located {
        revision_id: "revision-2".to_owned(),
        span: [0, 6_000],
    };
    assert_eq!(actual.as_slice(), slice::from_ref(&first));
    assert!(!proof_complete(&[vec![first.clone(), second]], &actual));
    assert!(proof_complete(&[vec![first]], &actual));
}

#[test]
fn graph_eval_empty_context_and_invalid_budget_never_earn_credit() {
    let mut input = bundle();
    input.passages.clear();
    input.budget.evidence_bytes = 0;
    assert!(anchors(input).unwrap().is_empty());
    let mut input = bundle();
    input.budget.limit = 1;
    assert!(anchors(input).is_err());
}
