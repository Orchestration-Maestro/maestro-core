//! The comparison across rungs: each floor per rung and its change, the
//! provenance of every rung, no change to or from an INVALID rung, and "not
//! run" for what a rung did not ask.

use super::{
    super::{
        comparison::Comparison,
        manifest::AskSettings,
        rung_prompt::RungPrompt,
        runner::{RungRun, run_ladder},
    },
    reports::{BINARY, runs, to_json},
    support::{FakeEngine, RERANKER, rung, suite},
};
use maestro_knowledge::{
    answer::PromptVersion,
    eval::{Measure, score_ladder},
};
use serde_json::{Value, json};
use std::time::Duration;

/// Every search of `run` took `search_us`, and its score says so.
fn searches_took(run: &mut RungRun, search_us: u64) {
    for row in &mut run.rows {
        row.search.elapsed = Duration::from_micros(search_us);
    }
    run.score = score_ladder(&suite(2, 1), &run.rows);
}

#[test]
fn the_comparison_gives_each_floor_per_rung_and_its_change() {
    let mut runs = runs();
    runs[1].score.floors[0].measure = Measure::Share {
        count: 1,
        of: 2,
        percent: 90,
        required: 2,
    };
    runs[1].score.supported_answers = 1;
    let suite = suite(2, 1);
    let comparison = Comparison::new(&runs, "docs", &suite.digest, BINARY);
    let json = to_json(&comparison);

    assert_eq!(json["schema"], "maestro-eval-ladder-comparison/1");
    assert_eq!(json["rungs"][0]["rung"], "r0");
    assert_eq!(json["rungs"][0]["floors"][0]["change"], Value::Null);
    assert_eq!(
        json["rungs"][1]["floors"][0],
        json!({"floor": "top_10", "status": "pass", "value": 1, "change": -1, "ran": true})
    );
    assert_eq!(json["rungs"][1]["supported_change"], -1);
    let markdown = comparison.to_markdown();
    let lines: Vec<&str> = markdown.lines().collect();
    assert!(lines[2].starts_with("| r0 | PASS | 2 PASS | 2 PASS | 1 PASS |"));
    assert!(lines[3].starts_with("| r1 | PASS | 1 PASS (-1) | 2 PASS (+0) |"));
    assert!(lines[3].ends_with(" 1 (-1) |"));
}

#[test]
fn the_comparison_names_the_provenance_of_every_rung() {
    let runs = runs();
    let suite = suite(2, 1);
    let comparison = Comparison::new(&runs, "docs", &suite.digest, BINARY);
    let json = to_json(&comparison);

    assert_eq!(json["collection"], "docs");
    assert_eq!(json["suite_digest"], suite.digest.as_str());
    assert_eq!(
        json["binary"],
        json!({"version": "0.1.0", "commit": "abc123"})
    );
    for (index, run) in runs.iter().enumerate() {
        assert_eq!(json["rungs"][index]["provenance"], to_json(&run.start));
        assert_eq!(json["rungs"][index]["end"], to_json(&run.end));
    }
    let markdown = comparison.to_markdown();
    assert!(markdown.contains("\n- Collection: docs\n"));
    assert!(markdown.contains(&format!("- Suite digest: {}\n", suite.digest.as_str())));
    assert!(markdown.contains("- Binary: 0.1.0 (abc123)\n"));
    let embedder = "e".repeat(64);
    let answerer = "a".repeat(64);
    assert!(markdown.contains(&format!(
        "- `r0`: generation 0, chunk set chunk-set, embedder card {embedder}, reranker card \
         {RERANKER}, answerer card {answerer}; ask: at most 5 passages, 6000 evidence bytes, 1024 \
         output tokens, prompt v2, search deadline 10000 ms\n"
    )));
    assert!(markdown.contains(&format!(
        "- `r1`: generation 0, chunk set chunk-set, embedder card {embedder}, reranker card \
         none, answerer card {answerer}; ask: at most 5 passages, 6000 evidence bytes, 1024 output \
         tokens, prompt v2, search deadline 10000 ms\n"
    )));
}

#[test]
fn the_comparison_names_each_rungs_ask_settings() {
    let mut runs = runs();
    runs[0].rung.ask = None;
    runs[1].rung.ask = Some(AskSettings {
        k: Some(8),
        max_tokens: Some(9000),
        output_tokens: Some(900),
        prompt: RungPrompt::Version(PromptVersion::V2),
        card: None,
    });
    let suite = suite(2, 1);
    let comparison = Comparison::new(&runs, "docs", &suite.digest, BINARY);
    let json = to_json(&comparison);

    assert_eq!(json["rungs"][0]["ask_settings"], Value::Null);
    assert_eq!(
        json["rungs"][1]["ask_settings"],
        json!({
            "k": 8,
            "max_tokens": 9000,
            "output_tokens": 900,
            "prompt": "v2",
            "search_deadline_ms": 10_000
        })
    );
    let markdown = comparison.to_markdown();
    let embedder = "e".repeat(64);
    let answerer = "a".repeat(64);
    assert!(markdown.contains(&format!(
        "- `r0`: generation 0, chunk set chunk-set, embedder card {embedder}, reranker card \
         {RERANKER}, answerer card {answerer}; no ask\n"
    )));
    assert!(markdown.contains(
        "; ask: at most 8 passages, 9000 evidence bytes, 900 output tokens, prompt v2, \
         search deadline 10000 ms\n"
    ));
}

#[test]
fn the_latency_cells_give_the_p95_and_its_change_in_milliseconds() {
    let mut runs = runs();
    searches_took(&mut runs[0], 1_234_567);
    searches_took(&mut runs[1], 1_235_567);
    let suite = suite(2, 1);
    let comparison = Comparison::new(&runs, "docs", &suite.digest, BINARY);
    let json = to_json(&comparison);

    assert_eq!(json["rungs"][1]["floors"][6]["value"], 1_235_567);
    assert_eq!(json["rungs"][1]["floors"][6]["change"], 1000);
    let markdown = comparison.to_markdown();
    let lines: Vec<&str> = markdown.lines().collect();
    assert!(lines[2].contains(" | 1234.567 ms PASS | "), "{}", lines[2]);
    assert!(
        lines[3].contains(" | 1235.567 ms PASS (+1.000 ms) | "),
        "{}",
        lines[3]
    );

    searches_took(&mut runs[1], 1_233_567);
    let comparison = Comparison::new(&runs, "docs", &suite.digest, BINARY);
    let markdown = comparison.to_markdown();
    assert!(markdown.contains(" | 1233.567 ms PASS (-1.000 ms) | "));
}

#[test]
fn a_rung_on_another_generation_is_invalid_and_has_no_change() {
    let mut engine = FakeEngine {
        drift_at_start_of: Some("r1".to_owned()),
        ..FakeEngine::default()
    };
    let rungs = [rung("r0"), rung("r1"), rung("r2")];
    let runs = run_ladder(&mut engine, &suite(2, 1), 0, &rungs, |_| Ok(())).unwrap();
    let suite = suite(2, 1);
    let comparison = Comparison::new(&runs, "docs", &suite.digest, BINARY);
    let json = to_json(&comparison);

    for index in 1..=2 {
        let rung = &json["rungs"][index];
        assert_eq!(rung["verdict"], "INVALID");
        assert_eq!(rung["supported_change"], Value::Null);
        for floor in rung["floors"].as_array().unwrap() {
            assert_eq!(floor["change"], Value::Null, "{floor}");
        }
    }
    let markdown = comparison.to_markdown();
    let lines: Vec<&str> = markdown.lines().collect();
    assert!(lines[3].starts_with("| r1 | INVALID | 2 PASS | 2 PASS | 1 PASS |"));
    assert!(!lines[3].contains('('));
    assert!(!lines[4].contains('('));
}

#[test]
fn a_rung_that_does_not_ask_shows_not_run_and_no_change() {
    let mut engine = FakeEngine::default();
    let mut retrieval = rung("r1");
    retrieval.ask = None;
    let runs = run_ladder(
        &mut engine,
        &suite(2, 1),
        0,
        &[rung("r0"), retrieval],
        |_| Ok(()),
    )
    .unwrap();
    let suite = suite(2, 1);
    let comparison = Comparison::new(&runs, "docs", &suite.digest, BINARY);
    let json = to_json(&comparison);

    let rung = &json["rungs"][1];
    for index in [2, 3, 4, 5, 7] {
        assert_eq!(
            (
                rung["floors"][index]["value"].clone(),
                rung["floors"][index]["change"].clone(),
                rung["floors"][index]["ran"].clone()
            ),
            (Value::Null, Value::Null, json!(false))
        );
    }
    assert_eq!(rung["supported_answers"], Value::Null);
    let markdown = comparison.to_markdown();
    let row = markdown.lines().nth(3).unwrap();
    assert!(
        row.starts_with(
            "| r1 | FAIL | 2 PASS (+0) | 2 PASS (+0) | not run | not run | not run | not run |"
        ),
        "{row}"
    );
    assert!(row.ends_with(" | not run | not run |"), "{row}");
}
