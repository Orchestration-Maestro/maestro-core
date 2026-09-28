//! What a ladder writes: private rows with IDs, ranks, citations, refusal
//! codes and timings; public reports with provenance and floors; and the
//! comparison across rungs. No file holds a question's text.

use super::{
    super::{
        comparison::{Comparison, write_comparison},
        reports::{Binary, PrivateRow, RungReport, write_rung},
        runner::{RungRun, SearchDiagnostic, run_ladder},
    },
    support::{FakeEngine, RERANKER, rung, suite},
};
use maestro_knowledge::{
    answer::RefusalCode,
    eval::{AskOutcome, SearchOutcome},
};
use serde_json::{Value, json};
use std::{env, fs, process, time::Duration};

/// The binary the tests' reports name.
pub(super) const BINARY: Binary = Binary {
    version: "0.1.0",
    commit: Some("abc123"),
};

/// The runs of the rungs `r0`, which reranks, and `r1`, which does not, over
/// two answerable questions and one unanswerable.
pub(super) fn runs() -> Vec<RungRun> {
    let mut engine = FakeEngine::default();
    let mut second = rung("r1");
    second.configuration.rerank = None;
    run_ladder(&mut engine, &suite(2, 1), 1, &[rung("r0"), second], |_| {
        Ok(())
    })
    .unwrap()
}

/// `value` as JSON.
pub(super) fn to_json(value: &impl serde::Serialize) -> Value {
    serde_json::to_value(value).unwrap()
}

#[test]
fn a_private_row_holds_ids_ranks_citations_refusals_and_timings() {
    let runs = runs();
    let mut row = runs[0].rows[0].clone();
    if let AskOutcome::Answered { citations, .. } = &mut row.ask.outcome {
        citations[0].revision_id = Some("revision-a0".to_owned());
        citations[0].chunk_id = Some("chunk-a0".to_owned());
        citations[0].span = Some([12, 34]);
    }
    let answered = to_json(&PrivateRow::new(
        &row,
        &runs[0].diagnostics[0],
        &runs[0].rejections[0],
        true,
    ));
    let refused = to_json(&PrivateRow::new(
        &runs[0].rows[2],
        &runs[0].diagnostics[2],
        &runs[0].rejections[2],
        true,
    ));

    let keys: Vec<&str> = answered
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "ask",
            "ask_us",
            "bundle_documents",
            "bundle_rank",
            "candidate_context_fallbacks",
            "candidate_source_load_micros",
            "citations",
            "delivered",
            "expected_rank",
            "id",
            "ranked_documents",
            "refusal",
            "rejections",
            "search",
            "search_us",
            "top_fused_score",
            "top_rerank_score"
        ]
    );
    assert_eq!(answered["id"], "a0");
    assert_eq!(answered["search"], "ranked");
    assert_eq!(answered["ranked_documents"], json!(["doc-a0"]));
    assert_eq!(answered["expected_rank"], 1);
    assert_eq!(answered["bundle_documents"], json!(["doc-a0"]));
    assert_eq!(answered["bundle_rank"], 1);
    assert_eq!(answered["ask"], "answered");
    assert_eq!(
        answered["citations"],
        json!([{
            "document_id": "doc-a0",
            "revision_id": "revision-a0",
            "chunk_id": "chunk-a0",
            "section_id": "section-a0",
            "span": [12, 34]
        }])
    );
    assert!(answered["search_us"].is_u64() && answered["ask_us"].is_u64());
    assert_eq!(refused["ask"], "refused");
    assert_eq!(refused["refusal"], "not_found");
    assert_eq!(refused["expected_rank"], Value::Null);
}

#[test]
fn a_private_row_carries_the_checks_the_answer_check_refused_without_tokens() {
    let runs = runs();
    let answered = to_json(&PrivateRow::new(
        &runs[0].rows[0],
        &runs[0].diagnostics[0],
        &runs[0].rejections[0],
        true,
    ));
    let refused = to_json(&PrivateRow::new(
        &runs[0].rows[2],
        &runs[0].diagnostics[2],
        &runs[0].rejections[2],
        true,
    ));

    assert_eq!(
        answered["rejections"],
        json!([{"attempt": 1, "check": "unsupported_literal"}])
    );
    assert_eq!(refused["rejections"], json!([]));
}

#[test]
fn a_private_row_carries_the_top_rerank_and_fused_scores() {
    let runs = runs();
    let reranked = to_json(&PrivateRow::new(
        &runs[0].rows[0],
        &runs[0].diagnostics[0],
        &runs[0].rejections[0],
        true,
    ));
    let fused_only = to_json(&PrivateRow::new(
        &runs[1].rows[0],
        &runs[1].diagnostics[0],
        &runs[1].rejections[0],
        true,
    ));

    assert_eq!(
        (
            reranked["top_rerank_score"].clone(),
            reranked["top_fused_score"].clone()
        ),
        (json!(0.75), json!(0.05))
    );
    assert_eq!(
        (
            fused_only["top_rerank_score"].clone(),
            fused_only["top_fused_score"].clone()
        ),
        (Value::Null, json!(0.05))
    );
}

#[test]
fn a_row_of_a_rung_that_does_not_ask_holds_no_ask() {
    let runs = runs();
    let mut row = runs[0].rows[2].clone();
    row.search.outcome = SearchOutcome::TimedOut;
    row.ask.outcome = AskOutcome::Refused(RefusalCode::NotFound);
    let unasked = to_json(&PrivateRow::new(
        &row,
        &SearchDiagnostic::default(),
        &[],
        false,
    ));

    assert_eq!(unasked["search"], "timed_out");
    assert_eq!(unasked["ranked_documents"], json!([]));
    assert_eq!(unasked["ask"], Value::Null);
    assert_eq!(unasked["ask_us"], Value::Null);
    assert_eq!(unasked["refusal"], Value::Null);
}

#[test]
fn a_rung_report_names_its_provenance_and_scores_its_floors() {
    let runs = runs();
    let suite = suite(2, 1);
    let report = RungReport::new(&runs[0], "docs", &suite.digest, BINARY);
    let mut json = to_json(&report);
    let score = json.as_object_mut().unwrap().remove("score").unwrap();
    let provenance = json!({
        "generation": 0,
        "chunk_set": "chunk-set",
        "embedder": "e".repeat(64),
        "reranker": RERANKER,
        "answerer": "a".repeat(64),
        "prompt": null,
    });

    assert_eq!(
        json,
        json!({
            "schema": "maestro-eval-ladder-rung/2",
            "rung": "r0",
            "verdict": "PASS",
            "ask": true,
            "ask_settings": {
                "k": 5,
                "max_tokens": 6000,
                "output_tokens": 1024,
                "prompt": "v2",
                "evidence": {"expansion":"full_section", "evidence_counter":"utf8"},
                "search_deadline_ms": 30_000
            },
            "warm_ups": 1,
            "collection": "docs",
            "provenance": provenance,
            "end": provenance,
            "ladder": provenance,
            "configuration": to_json(&runs[0].rung.configuration),
            "search_deadline_ms": 30_000,
            "suite_digest": suite.digest.as_str(),
            "binary": {"version": "0.1.0", "commit": "abc123"},
            "delivery": {
                "answerable": 2,
                "delivered": 2,
                "fully_delivered": 2,
                "median_coverage_permille": 1000,
                "composition_cases": 0,
                "compositions_delivered": 0,
                "uncredited": 0
            },
            "rejected_checks": {"unsupported_literal": 2},
        })
    );
    assert_eq!(score, to_json(&runs[0].score));
    assert_eq!(score["schema"], "maestro-eval-ladder/1");
}

#[test]
fn a_rung_report_in_markdown_names_its_provenance_then_its_floors() {
    let runs = runs();
    let suite = suite(2, 1);
    let markdown = RungReport::new(&runs[0], "docs", &suite.digest, BINARY).to_markdown();

    assert!(markdown.starts_with("# Ladder rung `r0`: PASS\n"));
    assert!(markdown.contains("- Generation: 0\n"));
    assert!(markdown.contains(&format!("- Reranker card: {RERANKER}\n")));
    assert!(markdown.contains("- Prompt file: none\n"));
    assert!(markdown.contains("- Binary: 0.1.0 (abc123)\n"));
    assert!(markdown.contains(concat!(
        "- Citation scoring: same document and pinned revision; exact section ID or a cited ",
        "span intersecting the expected section's extent (half-open, max(starts) < ",
        "min(ends); touching spans do not); a whole-document expectation matches any ",
        "citation of that revision.\n"
    )));
    assert!(markdown.contains("| Right document top-10 | 2/2 (100.0%) |"));
    assert!(markdown.contains(concat!(
        "- Scored bundle: the evidence each ask gave its answerer, under the ask settings\n",
        "- Delivered-section recall: 2/2 (same document"
    )));
    assert!(markdown.contains("- Fully delivered sections: 2/2 ("));
    assert!(markdown.contains("- Required-composition coverage: 0/0\n"));
    assert!(markdown.contains(concat!(
        "- Rejected answer attempts: unsupported_literal 2; the invented-literals floor ",
        "counts delivered answers only\n"
    )));
}

#[test]
fn an_invalid_rung_says_so_in_its_report() {
    let mut runs = runs();
    runs[0].end.as_mut().unwrap().generation = 7;
    let suite = suite(2, 1);
    let report = RungReport::new(&runs[0], "docs", &suite.digest, BINARY);

    assert_eq!(to_json(&report)["verdict"], "INVALID");
    assert_eq!(to_json(&report)["end"]["generation"], 7);
    assert!(
        report
            .to_markdown()
            .contains("- INVALID: the generation or a card changed while the rung ran, or")
    );
}

#[test]
fn the_files_hold_no_question_text() {
    let runs = runs();
    let suite = suite(2, 1);
    let output = env::temp_dir().join(format!("maestro-ladder-reports-{}", process::id()));
    for run in &runs {
        let report = RungReport::new(run, "docs", &suite.digest, BINARY);
        write_rung(&output, run, &report).unwrap();
    }
    let comparison = Comparison::new(&runs, "docs", &suite.digest, BINARY);
    write_comparison(&output, &comparison).unwrap();

    let mut files = Vec::new();
    for name in [
        "r0.json",
        "r0.md",
        "r1.json",
        "r1.md",
        "ladder.json",
        "ladder.md",
    ] {
        files.push(fs::read_to_string(output.join(name)).unwrap());
    }
    let rows = fs::read_to_string(output.join("private/r0.jsonl")).unwrap();
    assert_eq!(rows.lines().count(), 3);
    files.push(rows);
    for text in &files {
        assert!(!text.contains("secret"), "{text}");
    }
    assert!(!files[0].contains("doc-a0"));
    fs::remove_dir_all(output).unwrap();
}

#[test]
fn a_private_row_gives_its_times_in_microseconds() {
    let runs = runs();
    let mut row = runs[0].rows[0].clone();
    row.search.elapsed = Duration::from_micros(1_234_567);
    row.ask.elapsed = Duration::from_micros(7_654_321);
    let json = to_json(&PrivateRow::new(
        &row,
        &SearchDiagnostic::default(),
        &[],
        true,
    ));

    assert_eq!(
        (json["search_us"].clone(), json["ask_us"].clone()),
        (json!(1_234_567), json!(7_654_321))
    );
}

#[test]
fn a_right_document_ranked_7th_but_not_assembled_is_in_the_top_10() {
    let mut engine = FakeEngine {
        right_document_at_7: true,
        ..FakeEngine::default()
    };
    let runs = run_ladder(&mut engine, &suite(2, 1), 0, &[rung("r0")], |_| Ok(())).unwrap();
    let run = &runs[0];
    let row = to_json(&PrivateRow::new(
        &run.rows[0],
        &run.diagnostics[0],
        &run.rejections[0],
        true,
    ));

    assert_eq!(row["expected_rank"], 7);
    assert_eq!(row["ranked_documents"].as_array().unwrap().len(), 10);
    assert_eq!(row["bundle_documents"].as_array().unwrap().len(), 3);
    assert_eq!(row["bundle_rank"], Value::Null);
    let floors = &to_json(&run.score)["floors"];
    assert_eq!(
        (
            floors[0]["floor"].clone(),
            floors[0]["status"].clone(),
            floors[0]["count"].clone()
        ),
        (json!("top_10"), json!("pass"), json!(2))
    );
    assert_eq!(
        (
            floors[1]["floor"].clone(),
            floors[1]["status"].clone(),
            floors[1]["count"].clone()
        ),
        (json!("top_1"), json!("fail"), json!(0))
    );
}
