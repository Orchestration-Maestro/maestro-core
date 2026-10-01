//! Expected section copies share one nDCG item while remaining visible in
//! suite and report data.

use super::support::{COLLECTION, GENERATION, bundle, close, hit, whole};
use crate::{
    eval::{
        Header, QuestionResult, RunError,
        metric::{DISCOUNTS, measure},
        run,
    },
    suite::Suite,
};
use maestro_canonicalization::{CanonicalizeInput, canonicalize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, convert::Infallible};

/// A one-question suite line, with `group` omitted when `None`.
fn suite_line(group: Option<&str>) -> String {
    let mut expected = json!({
        "source_ref": "https://handbook.example.org/document",
        "heading_path": ["Handbook", "Answer"],
    });
    if let Some(group) = group {
        expected["group"] = json!(group);
    }
    json!({
        "schema": "maestro-suite/1",
        "id": "copies",
        "language": "en",
        "question": "What answers this?",
        "answerable": true,
        "expected": [expected],
    })
    .to_string()
}

/// A question result whose three copies may share a group, with the listed
/// expected-section ranks.
fn result(grouped: bool, ranks: [Option<u32>; 4]) -> QuestionResult {
    let expected: Vec<Value> = ["copy-a", "copy-b", "copy-c", "other"]
        .into_iter()
        .zip(ranks)
        .enumerate()
        .map(|(index, (section_id, rank))| {
            let mut expected = json!({
                "document_id": "document",
                "section_id": section_id,
                "rank": rank,
            });
            if grouped && index < 3 {
                expected["group"] = json!("same-answer");
            }
            expected
        })
        .collect();
    serde_json::from_value(json!({
        "id": "copies",
        "answerable": true,
        "expected": expected,
        "passages": 3,
        "latency_us": 1,
        "degraded": false,
        "failures": [],
    }))
    .unwrap()
}

fn ndcg(grouped: bool, ranks: [Option<u32>; 4]) -> f64 {
    measure(&[result(grouped, ranks)], 1)
        .ndcg_at_10
        .unwrap()
        .value
}

#[test]
fn suite_expected_sections_accept_a_nonblank_group() {
    assert!(suite_line(Some("same-answer")).parse::<Suite>().is_ok());
}

#[test]
fn suite_expected_sections_reject_a_blank_group() {
    let error = suite_line(Some(" \t ")).parse::<Suite>().unwrap_err();
    assert!(error.to_string().contains("group must not be blank"));
}

#[test]
fn suite_expected_sections_accept_a_component_beside_a_group_and_refuse_a_blank_one() {
    let with = |component: &str| {
        let mut line: Value = serde_json::from_str(&suite_line(Some("same-answer"))).unwrap();
        line["expected"][0]["component"] = json!(component);
        line.to_string().parse::<Suite>()
    };

    let suite = with("condition").unwrap();
    let expected = &suite.questions[0].expected[0];
    assert_eq!(
        (expected.group.as_deref(), expected.component.as_deref()),
        (Some("same-answer"), Some("condition"))
    );
    let error = with("  ").unwrap_err();
    assert!(error.to_string().contains("component must not be blank"));
    let unnamed: Suite = suite_line(None).parse().unwrap();
    assert_eq!(unnamed.questions[0].expected[0].component, None);
}

#[test]
fn suite_expected_sections_accept_a_missing_group() {
    assert!(suite_line(None).parse::<Suite>().is_ok());
}

#[test]
fn ndcg_counts_three_copies_as_one_expected_item() {
    let score = ndcg(true, [Some(1), None, None, None]);
    close(score, 1.0 / (1.0 + DISCOUNTS[1]));
}

#[test]
fn ndcg_uses_the_best_rank_of_all_copies_in_a_group() {
    let one_copy = ndcg(true, [Some(1), None, None, None]);
    let all_copies = ndcg(true, [Some(3), Some(1), Some(2), None]);
    close(all_copies, one_copy);
}

#[test]
fn recall_and_mrr_count_any_member_of_a_group() {
    let metrics = measure(&[result(true, [None, Some(3), None, None])], 1);
    close(metrics.recall_at_5.unwrap().value, 1.0);
    close(metrics.mrr_at_10.unwrap().value, 1.0 / 3.0);
}

#[test]
fn ungrouped_expected_sections_keep_the_existing_ndcg_value() {
    let score = ndcg(false, [Some(1), None, None, None]);
    let ideal: f64 = DISCOUNTS.iter().take(4).sum();
    close(score, 1.0 / ideal);
}

#[test]
fn a_run_carries_the_suite_group_into_its_report() {
    const SOURCE: &str = "https://handbook.example.org/document";
    let mut input =
        CanonicalizeInput::new("# Handbook\n\n## Answer\n\nThe answer.\n", "handbook.md");
    input.metadata.source_reference = Some(SOURCE.to_owned());
    let document = canonicalize(input).unwrap();
    let section_id = document.sections[0].section_id.clone().leak();
    let suite: Suite = suite_line(Some("same-answer")).parse().unwrap();
    let report = run(
        Header {
            suite: "groups".to_owned(),
            collection: COLLECTION.to_owned(),
            generation: GENERATION,
            profiles: BTreeMap::new(),
            seed: 1,
        },
        &suite,
        |source_ref| Ok::<_, Infallible>((source_ref == SOURCE).then(|| document.clone())),
        |_| Ok::<_, Infallible>(bundle(&[hit(1, section_id, 1.0)])),
    )
    .unwrap();
    let value = serde_json::to_value(report).unwrap();
    assert_eq!(value["questions"][0]["expected"][0]["group"], "same-answer");
}

fn suite_of_copy_names(first_group: Option<&str>, second_group: Option<&str>) -> Suite {
    let mut first = json!({
        "source_ref": "https://handbook.example.org/copy-a",
        "heading_path": ["Handbook", "Answer"],
    });
    let mut second = json!({
        "source_ref": "https://handbook.example.org/copy-b",
        "heading_path": ["Handbook", "Answer"],
    });
    if let Some(group) = first_group {
        first["group"] = json!(group);
    }
    if let Some(group) = second_group {
        second["group"] = json!(group);
    }
    json!({
        "schema": "maestro-suite/1",
        "id": "copies",
        "language": "en",
        "question": "What answers this?",
        "answerable": true,
        "expected": [first, second],
    })
    .to_string()
    .parse()
    .unwrap()
}

#[test]
fn names_in_one_group_may_resolve_to_the_same_section() {
    const SOURCE: &str = "https://handbook.example.org/document";
    let mut input =
        CanonicalizeInput::new("# Handbook\n\n## Answer\n\nThe answer.\n", "handbook.md");
    input.metadata.source_reference = Some(SOURCE.to_owned());
    let document = canonicalize(input).unwrap();
    let section_id = document.sections[0].section_id.clone().leak();
    let suite = suite_of_copy_names(Some("glossary"), Some("glossary"));
    let report = run(
        Header {
            suite: "groups".to_owned(),
            collection: COLLECTION.to_owned(),
            generation: GENERATION,
            profiles: BTreeMap::new(),
            seed: 1,
        },
        &suite,
        |_| Ok::<_, Infallible>(Some(document.clone())),
        |_| Ok::<_, Infallible>(bundle(&[hit(1, section_id, 1.0)])),
    )
    .unwrap();
    let expected = &report.questions[0].expected;
    assert_eq!(expected.len(), 2);
    assert_eq!(expected[0].group.as_deref(), Some("glossary"));
    assert_eq!(expected[1].group.as_deref(), Some("glossary"));
    assert_eq!(expected[0].section_id, expected[1].section_id);
    assert_eq!(expected[0].rank, expected[1].rank);
}

#[test]
fn names_in_different_groups_may_not_resolve_to_the_same_section() {
    const SOURCE: &str = "https://handbook.example.org/document";
    let mut input =
        CanonicalizeInput::new("# Handbook\n\n## Answer\n\nThe answer.\n", "handbook.md");
    input.metadata.source_reference = Some(SOURCE.to_owned());
    let document = canonicalize(input).unwrap();
    let section_id = document.sections[0].section_id.clone().leak();
    let suite = suite_of_copy_names(Some("glossary"), Some("another-answer"));
    let error = run(
        Header {
            suite: "groups".to_owned(),
            collection: COLLECTION.to_owned(),
            generation: GENERATION,
            profiles: BTreeMap::new(),
            seed: 1,
        },
        &suite,
        |_| Ok::<_, Infallible>(Some(document.clone())),
        |_| Ok::<_, Infallible>(bundle(&[hit(1, section_id, 1.0)])),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::SameSection { question, .. } if question == "copies"));
}

#[test]
fn a_grouped_and_ungrouped_name_of_one_section_are_refused() {
    const SOURCE: &str = "https://handbook.example.org/document";
    let mut input =
        CanonicalizeInput::new("# Handbook\n\n## Answer\n\nThe answer.\n", "handbook.md");
    input.metadata.source_reference = Some(SOURCE.to_owned());
    let document = canonicalize(input).unwrap();
    let section_id = document.sections[0].section_id.clone().leak();
    let suite = suite_of_copy_names(None, Some("glossary"));
    let error = run(
        Header {
            suite: "groups".to_owned(),
            collection: COLLECTION.to_owned(),
            generation: GENERATION,
            profiles: BTreeMap::new(),
            seed: 1,
        },
        &suite,
        |_| Ok::<_, Infallible>(Some(document.clone())),
        |_| Ok::<_, Infallible>(bundle(&[hit(1, section_id, 1.0)])),
    )
    .unwrap_err();
    assert!(matches!(error, RunError::SameSection { question, .. } if question == "copies"));
}

#[test]
fn names_in_one_group_may_resolve_to_the_same_document() {
    const SOURCE: &str = "https://handbook.example.org/document";
    let mut input = CanonicalizeInput::new("An unsectioned document.\n", "document.md");
    input.metadata.source_reference = Some(SOURCE.to_owned());
    let document = canonicalize(input).unwrap();
    assert!(document.sections.is_empty());
    let document_id: &'static str = document.document_id.clone().leak();
    let suite: Suite = json!({
        "schema": "maestro-suite/1",
        "id": "copies",
        "language": "en",
        "question": "What answers this?",
        "answerable": true,
        "expected": [
            {
                "source_ref": "https://handbook.example.org/copy-a",
                "heading_path": [],
                "group": "glossary"
            },
            {
                "source_ref": "https://handbook.example.org/copy-b",
                "heading_path": [],
                "group": "glossary"
            },
        ],
    })
    .to_string()
    .parse()
    .unwrap();
    let report = run(
        Header {
            suite: "groups".to_owned(),
            collection: COLLECTION.to_owned(),
            generation: GENERATION,
            profiles: BTreeMap::new(),
            seed: 1,
        },
        &suite,
        |_| Ok::<_, Infallible>(Some(document.clone())),
        |_| Ok::<_, Infallible>(bundle(&[whole(1, document_id, 1.0)])),
    )
    .unwrap();
    let expected = &report.questions[0].expected;
    assert_eq!(expected.len(), 2);
    assert!(expected.iter().all(|item| item.document_id == document_id));
    assert!(
        expected
            .iter()
            .all(|item| item.group.as_deref() == Some("glossary"))
    );
    assert_eq!(expected[0].rank, expected[1].rank);
}
