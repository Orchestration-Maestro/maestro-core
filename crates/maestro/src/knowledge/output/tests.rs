use super::SearchTruncation;
use super::policy::shared_search_result_fits;
use super::{response_fits, search_tool_result, truncate_search_bundle};
use maestro_kernel::{
    artifact::Digest,
    evidence::{
        Budget, Bundle, Conflict, Inventory, InventoryCount, Passage, RouteStatus, Schema, Span,
        Trace,
    },
};
use rmcp::model::{CallToolResult, ProtocolVersion, RequestId};
use serde_json::Value;
use std::collections::BTreeMap;

#[test]
fn truncation_keeps_the_highest_ranked_real_passage_and_counts_drops() {
    let bundle = bundle(
        vec![
            passage(1, "Top passage — résumé ✓"),
            passage(2, &"é".repeat(3_000)),
        ],
        450,
    );
    assert!(
        !shared_search_result_fits(&bundle, SearchTruncation::default())
            .expect("initial wire size")
    );

    let bounded = truncate_search_bundle(bundle).expect("bounded search bundle");

    assert!(bounded.truncation.is_truncated());
    assert_eq!(bounded.truncation.passages, 1);
    assert_eq!(bounded.truncation.inventory_items, 0);
    assert_eq!(
        bounded
            .bundle
            .passages
            .iter()
            .map(|item| item.n)
            .collect::<Vec<_>>(),
        [1]
    );
    assert!(bounded.bundle.conflicts.is_empty());
    assert_eq!(
        bounded
            .bundle
            .trace
            .iter()
            .map(|item| item.n)
            .collect::<Vec<_>>(),
        [1]
    );
    assert_eq!(
        bounded.bundle.budget.evidence_tokens as usize,
        serde_json::to_vec(&bounded.bundle.passages).unwrap().len()
    );
    assert!(serde_json::to_value(&bounded.bundle).is_ok());

    let result = search_tool_result(&bounded.bundle, bounded.truncation)
        .expect("MCP result with reduced JSON and warning");
    assert_reduced_text_and_warning(&result);
}

#[test]
fn truncation_removes_only_the_gap_for_the_dropped_passage() {
    let mut bundle = bundle(
        vec![passage(1, "retained"), passage(2, &"é".repeat(3_000))],
        450,
    );
    bundle.known_gaps = vec![
        "Passage 1 has no source reference.".to_owned(),
        "Passage 2 has no source reference.".to_owned(),
        "An unrelated gap.".to_owned(),
    ];

    let bounded = truncate_search_bundle(bundle).expect("bounded search bundle");

    assert_eq!(bounded.truncation.passages, 1);
    assert_eq!(
        bounded.bundle.known_gaps,
        [
            "Passage 1 has no source reference.",
            "An unrelated gap.",
            "1 lowest-ranked passages were omitted to fit the response limit.",
        ]
    );
}

#[test]
fn successive_drops_keep_the_remaining_conflict_and_one_current_truncation_gap() {
    let large = "é".repeat(2_600);
    let mut bundle = bundle(
        vec![
            passage(1, "retained"),
            passage(2, "also retained"),
            passage(3, &large),
            passage(4, &large),
        ],
        450,
    );
    bundle.known_gaps = vec![
        "Passage 4 has no source reference.".to_owned(),
        "An unrelated gap.".to_owned(),
    ];

    let bounded = truncate_search_bundle(bundle).expect("bounded search bundle");

    assert_eq!(bounded.truncation.passages, 2);
    assert_eq!(bounded.bundle.conflicts.len(), 1);
    assert_eq!(bounded.bundle.conflicts[0].passages, [1, 2]);
    assert_eq!(
        bounded.bundle.known_gaps,
        [
            "An unrelated gap.",
            "2 lowest-ranked passages were omitted to fit the response limit.",
        ]
    );
}

#[test]
fn inventory_is_reduced_only_after_all_oversized_passages_are_dropped() {
    let total_groups = 560;
    let bundle = bundle(
        vec![
            passage(1, "Top passage — résumé ✓"),
            passage(2, &"é".repeat(3_000)),
        ],
        total_groups,
    );
    assert!(
        !shared_search_result_fits(&bundle, SearchTruncation::default())
            .expect("initial wire size")
    );

    let bounded = truncate_search_bundle(bundle).expect("bounded search bundle");

    assert_eq!(bounded.truncation.passages, 2);
    assert!(bounded.truncation.inventory_items > 0);
    assert!(bounded.bundle.passages.is_empty());
    let Some(Inventory::DocumentsBySet {
        total_documents,
        sets,
        ..
    }) = bounded.bundle.inventory.as_ref()
    else {
        panic!("reduced document inventory");
    };
    assert_eq!(*total_documents, total_groups as u64);
    assert_eq!(
        sets.len() + bounded.truncation.inventory_items,
        total_groups
    );
    assert_eq!(bounded.bundle.budget.evidence_tokens, 0);
    assert!(
        bounded
            .bundle
            .known_gaps
            .iter()
            .any(|gap| gap.starts_with("Search inventory truncated:"))
    );
    assert!(
        shared_search_result_fits(&bounded.bundle, bounded.truncation).expect("reduced wire size")
    );

    let result = search_tool_result(&bounded.bundle, bounded.truncation)
        .expect("MCP result with reduced JSON and warning");
    assert_reduced_text_and_warning(&result);
    let value = serde_json::to_value(&result).expect("serialize reduced result");
    assert_eq!(
        value["_meta"]["maestro/truncation"]["dropped"]["passages"],
        2
    );
    assert_eq!(
        value["_meta"]["maestro/truncation"]["dropped"]["inventory_items"],
        bounded.truncation.inventory_items
    );
}

fn assert_reduced_text_and_warning(result: &CallToolResult) {
    let value = serde_json::to_value(result).expect("serialize tool result");
    let text: Value = serde_json::from_str(value["content"][0]["text"].as_str().unwrap())
        .expect("parse reduced text-only result");
    assert_eq!(text, value["structuredContent"]);
    assert!(
        value["content"][1]["text"]
            .as_str()
            .unwrap()
            .contains("not exhaustive")
    );
    assert!(
        response_fits(
            result,
            &RequestId::Number(1),
            Some(&ProtocolVersion::V_2026_07_28)
        )
        .unwrap()
    );
}

fn bundle(passages: Vec<Passage>, group_count: usize) -> Bundle {
    let evidence_tokens = u32::try_from(serde_json::to_vec(&passages).unwrap().len()).unwrap();
    let groups = (0..group_count)
        .map(|number| InventoryCount {
            value: Some(format!("{number:04}-{}", "x".repeat(26))),
            documents: 1,
        })
        .collect::<Vec<_>>();
    Bundle {
        schema: Schema::V1,
        collection: "collection".to_owned(),
        generation: 1,
        query: "question".to_owned(),
        lang: "en".to_owned(),
        routes: BTreeMap::from([("structured".to_owned(), RouteStatus::Ok)]),
        conflicts: (passages.len() >= 2)
            .then(|| Conflict {
                entity: "service".to_owned(),
                attribute: "port".to_owned(),
                passages: passages.iter().map(|item| item.n).collect(),
            })
            .into_iter()
            .collect(),
        known_gaps: Vec::new(),
        budget: Budget {
            evidence_tokens,
            limit: 12_000,
            counter: Some("evidence-utf8-bytes/1".to_owned()),
            estimated: true,
        },
        request_budget: None,
        inventory: Some(Inventory::DocumentsBySet {
            set_filter: None,
            total_documents: groups.len() as u64,
            sets: groups,
        }),
        trace: passages.iter().map(|item| trace(item.n)).collect(),
        passages,
    }
}

fn passage(number: u32, text: &str) -> Passage {
    Passage {
        n: number,
        section_id: Some(format!("section-{number}")),
        document_id: format!("document-{number}"),
        revision_id: format!("revision-{number}"),
        title: "Guide".to_owned(),
        section_path: vec!["Guide".to_owned()],
        version: None,
        source_ref: "corpus-path:guide.md".to_owned(),
        span: Span {
            start: 0,
            end: text.len(),
        },
        digest: Digest::of(text.as_bytes()),
        text: text.to_owned(),
        windowed: false,
        alternates: Vec::new(),
    }
}

fn trace(number: u32) -> Trace {
    Trace {
        parent_context_of: Vec::new(),
        n: number,
        score: None,
        routes: vec!["identifier".to_owned()],
        chunk_ids: vec![format!("chunk-{number}")],
        procedural: false,
    }
}
