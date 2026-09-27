//! Hand-checked v2 ranking, subgroup and repeated-question metrics.

use super::{
    support::{bundle, hit},
    v2::header_v2,
};
use crate::{
    eval::{ItemStatus, Subgroup, merge_v2_attempts, run_v2},
    suite::Suite,
};
use maestro_canonicalization::{CanonicalDocument, CanonicalizeInput, canonicalize};
use maestro_kernel::evidence::Bundle;
use serde_json::Value;

const SCORING_SOURCE: &str = "https://handbook.example.org/t030b-scoring";

#[test]
fn v2_metrics_hand_check_grouped_ungrouped_failed_and_subgroup_scores() {
    let (suite, document) = scoring_fixture();
    let (section_a, section_b, section_c, section_solo) = (
        section_id(&document, "A"),
        section_id(&document, "B"),
        section_id(&document, "C"),
        section_id(&document, "Solo"),
    );
    let mut header = header_v2();
    header.cross_lingual_questions.insert("grouped".to_owned());
    let report = run_v2(
        header,
        &suite,
        |source_ref| Ok::<_, &str>((source_ref == SCORING_SOURCE).then(|| document.clone())),
        |question| match question.id.as_str() {
            "grouped" => Ok(scored_bundle(
                &document.document_id,
                &[
                    hit(1, section_b, 4.0),
                    hit(2, section_a, 3.0),
                    hit(3, section_solo, 2.0),
                    hit(4, section_c, 1.0),
                ],
            )),
            "ungrouped" => Ok(scored_bundle(
                &document.document_id,
                &[hit(1, section_b, 2.0), hit(2, section_a, 1.0)],
            )),
            "failed" => Err("timeout"),
            "no-answer" => Ok(bundle(&[])),
            other => panic!("unexpected question {other}"),
        },
    )
    .unwrap();

    let grouped_ndcg = 1.5 / (1.0 + 1.0 / 3.0_f64.log2());
    let ungrouped_ndcg = 1.0 / 3.0_f64.log2();
    close(report.metrics.recall_at_5.unwrap().value, 2.0 / 3.0);
    close(report.metrics.mrr_at_10.unwrap().value, 0.5);
    close(
        report.metrics.ndcg_at_10.unwrap().value,
        (grouped_ndcg + ungrouped_ndcg) / 3.0,
    );
    close(report.metrics.no_answer_accuracy.unwrap().value, 1.0);
    assert!(matches!(
        report.questions.iter().find(|row| row.id == "failed").unwrap().status,
        Some(ItemStatus::Failed { ref reason, .. }) if reason == "timeout"
    ));
    let subgroups = report.subgroups.unwrap();
    assert_eq!(subgroups[&Subgroup::En].question_count, 3);
    assert_eq!(subgroups[&Subgroup::En].answerable_questions, 2);
    close(
        subgroups[&Subgroup::En].metrics.recall_at_5.unwrap().value,
        0.5,
    );
    close(
        subgroups[&Subgroup::En]
            .metrics
            .no_answer_accuracy
            .unwrap()
            .value,
        1.0,
    );
    assert_eq!(subgroups[&Subgroup::Fr].question_count, 1);
    close(
        subgroups[&Subgroup::Fr].metrics.mrr_at_10.unwrap().value,
        0.5,
    );
    assert_eq!(subgroups[&Subgroup::CrossLingual].question_count, 1);
    close(
        subgroups[&Subgroup::CrossLingual]
            .metrics
            .recall_at_5
            .unwrap()
            .value,
        1.0,
    );
}

#[test]
fn three_v2_repetitions_stay_clustered_by_question() {
    let (suite, document) = scoring_fixture();
    let (section_a, section_b, section_solo) = (
        section_id(&document, "A"),
        section_id(&document, "B"),
        section_id(&document, "Solo"),
    );
    let mut reports = Vec::new();
    for repetition in 1..=3 {
        let mut header = header_v2();
        header.planned_repetitions = 3;
        header.attempt = repetition;
        header.repetition = repetition;
        header.attempt_seed = 22 + u64::from(repetition);
        header.cross_lingual_questions.insert("grouped".to_owned());
        reports.push(
            run_v2(
                header,
                &suite,
                |source_ref| {
                    Ok::<_, &str>((source_ref == SCORING_SOURCE).then(|| document.clone()))
                },
                |question| match (question.id.as_str(), repetition) {
                    ("grouped", 1) => Ok(scored_bundle(
                        &document.document_id,
                        &[hit(1, section_b, 2.0), hit(3, section_solo, 1.0)],
                    )),
                    ("grouped" | "ungrouped", 2) | ("no-answer", _) => Ok(bundle(&[])),
                    ("grouped", 3) => Ok(scored_bundle(
                        &document.document_id,
                        &[hit(2, section_b, 1.0)],
                    )),
                    ("ungrouped", 1) => Ok(scored_bundle(
                        &document.document_id,
                        &[hit(1, section_b, 2.0), hit(2, section_a, 1.0)],
                    )),

                    ("ungrouped", 3) => Ok(scored_bundle(
                        &document.document_id,
                        &[hit(1, section_a, 1.0)],
                    )),
                    ("failed", _) => Err("timeout"),

                    (other, _) => panic!("unexpected question {other}"),
                },
            )
            .unwrap(),
        );
    }

    let merged = merge_v2_attempts(&reports).unwrap();
    assert_eq!(merged.questions.len(), 12);
    close(merged.metrics.recall_at_5.unwrap().value, 4.0 / 9.0);
    close(merged.metrics.mrr_at_10.unwrap().value, 7.0 / 18.0);
    let cross_lingual = &merged.subgroups.as_ref().unwrap()[&Subgroup::CrossLingual];
    assert_eq!(cross_lingual.question_count, 1);
    assert_eq!(cross_lingual.attempt_count, 3);
    let recall = cross_lingual.metrics.recall_at_5.unwrap();
    close(recall.value, 2.0 / 3.0);
    assert_eq!(recall.low.to_bits(), recall.value.to_bits());
    assert_eq!(recall.high.to_bits(), recall.value.to_bits());
}

/// The v2 suite and canonical source document used for score calculations.
fn scoring_fixture() -> (Suite, CanonicalDocument) {
    let markdown = concat!(
        "# Handbook\n\n",
        "## A\n\nAnswer A.\n\n",
        "## B\n\nAnswer B.\n\n",
        "## C\n\nAnswer C.\n\n",
        "## Solo\n\nAnswer Solo.\n"
    );
    let mut input = CanonicalizeInput::new(markdown, "handbook.md");
    input.metadata.source_reference = Some(SCORING_SOURCE.to_owned());
    let document = canonicalize(input).unwrap();
    let expected = |title: &str| {
        serde_json::json!({
            "source_ref": SCORING_SOURCE,
            "heading_path": ["Handbook", title],
        })
    };
    let mut grouped_a = expected("A");
    let mut grouped_b = expected("B");
    let mut grouped_c = expected("C");
    for member in [&mut grouped_a, &mut grouped_b, &mut grouped_c] {
        member["group"] = serde_json::json!("same-answer");
    }
    let lines = [
        serde_json::json!({
            "schema": "maestro-suite/1", "id": "grouped", "language": "en",
            "question": "Which sections answer the grouped question?", "answerable": true,
            "expected": [grouped_a, grouped_b, grouped_c, expected("Solo")],
        }),
        serde_json::json!({
            "schema": "maestro-suite/1", "id": "ungrouped", "language": "fr",
            "question": "Quelle section répond?", "answerable": true,
            "expected": [expected("A")],
        }),
        serde_json::json!({
            "schema": "maestro-suite/1", "id": "failed", "language": "en",
            "question": "Which section answers?", "answerable": true,
            "expected": [expected("C")],
        }),
        serde_json::json!({
            "schema": "maestro-suite/1", "id": "no-answer", "language": "en",
            "question": "Is this answerable?", "answerable": false,
            "expected": [],
        }),
    ];
    let suite = lines
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        .parse()
        .unwrap();
    (suite, document)
}

/// Returns one section's stable ID as a test-only hit label.
fn section_id(document: &CanonicalDocument, title: &str) -> &'static str {
    Box::leak(
        document
            .sections
            .iter()
            .find(|section| section.title == title)
            .unwrap()
            .section_id
            .clone()
            .into_boxed_str(),
    )
}

/// Makes support hits belong to the resolved canonical document.
fn scored_bundle(document_id: &str, hits: &[super::support::Hit]) -> Bundle {
    let mut result = bundle(hits);
    for passage in &mut result.passages {
        passage.document_id = document_id.to_owned();
    }
    result
}

/// Compares floating-point metric output within one machine epsilon.
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < f64::EPSILON,
        "actual {actual:?} != expected {expected:?}"
    );
}
