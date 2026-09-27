//! Report v2 runs, validation, and v1 compatibility.

use super::super::run_v2::{RetrievalObservation, TrialEvidence, run_v2_authorized};
use super::support::{COLLECTION, GENERATION, bundle, question, report_of};
use crate::{
    eval::{
        AggregateError, AttemptEvent, Header, HeaderV2, ItemStatus, MeasurementCohort, Report,
        RunError, Schema, Subgroup, TrialMode, judge::judge, run_v2, run_v2_observed,
    },
    prepare::QualificationMode,
    suite::{Question, Suite},
};
use maestro_kernel::{artifact::Digest, evidence::Bundle};
use serde_json::Value;
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

#[test]
fn public_v2_runner_always_records_synthetic_mode() {
    let report = run_v2(
        header_v2(),
        &suite(&["question"]),
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();

    assert_eq!(report.mode, Some(TrialMode::Synthetic));
}

#[test]
fn synthetic_qualification_cannot_authorize_real_report_evidence() {
    let refused =
        TrialEvidence::authorized(QualificationMode::Synthetic, TrialMode::Real).unwrap_err();

    assert_eq!(
        refused,
        "real evaluation requires native tokenizer qualification"
    );
}

#[test]
fn v2_run_keeps_failures_and_wrong_generation_rows_and_continues() {
    let suite = suite(&["failed", "empty", "wrong-generation"]);
    let mut retrieved = Vec::new();
    let report = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |question| answer_mixed(question, &mut retrieved),
    )
    .unwrap();

    assert_eq!(retrieved, ["failed", "empty", "wrong-generation"]);
    assert_eq!(report.schema, Schema::V2);
    assert_eq!(report.integrity_violation, Some(true));
    assert_failed_items(&report);
    assert_close(report.metrics.no_answer_accuracy.unwrap().value, 1.0 / 3.0);
    assert_eq!(report.questions[0].attempt, Some(1));
    assert_eq!(report.questions[0].seed, Some(23));
    assert_eq!(report.questions[0].route.as_deref(), Some("dense"));
    assert_eq!(report.questions[0].cohort, Some(MeasurementCohort::Unknown));
    assert_eq!(report.questions[0].warm_up, Some(false));
    assert_eq!(
        report.subgroups.as_ref().unwrap()[&Subgroup::En].question_count,
        3
    );
    assert_eq!(
        report.subgroups.as_ref().unwrap()[&Subgroup::Fr].question_count,
        0
    );
}

#[test]
fn durable_observer_records_start_before_route_and_stops_after_write_failure() {
    let suite = suite(&["first", "second"]);
    let mut retrieved = Vec::new();
    let mut events = Vec::new();
    let result = run_v2_observed(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |question| {
            retrieved.push(question.id.clone());
            Ok(bundle(&[]))
        },
        |event| match event {
            AttemptEvent::Started { question, .. } => {
                events.push(format!("start:{}", question.id));
                if question.id == "second" {
                    Err("journal unavailable".to_owned())
                } else {
                    Ok(())
                }
            }
            AttemptEvent::Completed { question, .. } => {
                events.push(format!("complete:{}", question.id));
                Ok(())
            }
        },
    );
    assert!(matches!(
        result,
        Err(RunError::Recording { ref question, ref reason })
            if question == "second" && reason == "journal unavailable"
    ));
    assert_eq!(events, ["start:first", "complete:first", "start:second"]);
    assert_eq!(retrieved, ["first"]);
}

#[test]
fn v2_reader_recomputes_all_row_derived_summaries() {
    let suite = suite(&["wrong-generation", "empty"]);
    let report = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |question| {
            let mut bundle = bundle(&[]);
            if question.id == "wrong-generation" {
                bundle.generation = GENERATION + 1;
            }
            Ok(bundle)
        },
    )
    .unwrap();
    let serialized = serde_json::to_string(&report).unwrap();
    assert_eq!(Report::from_str(&serialized).unwrap(), report);
    let value: Value = serde_json::from_str(&serialized).unwrap();

    let mut inconsistent = value.clone();
    inconsistent["integrity_violation"] = Value::Bool(false);
    assert!(Report::from_str(&inconsistent.to_string()).is_err());

    let mut inconsistent = value.clone();
    inconsistent["degraded_searches"] = Value::from(report.degraded_searches + 1);
    assert!(Report::from_str(&inconsistent.to_string()).is_err());

    let mut inconsistent = value.clone();
    let accuracy = inconsistent["metrics"]["no_answer_accuracy"]["value"]
        .as_f64()
        .unwrap();
    inconsistent["metrics"]["no_answer_accuracy"]["value"] = serde_json::json!(accuracy + 0.125);
    assert!(Report::from_str(&inconsistent.to_string()).is_err());

    let mut inconsistent = value.clone();
    inconsistent["cohorts"]["unknown"]["attempts"] = Value::from(0);
    assert!(Report::from_str(&inconsistent.to_string()).is_err());

    let mut inconsistent = value;
    inconsistent["subgroups"]["en"]["question_count"] = Value::from(0);
    assert!(Report::from_str(&inconsistent.to_string()).is_err());
}

#[test]
fn v2_cohort_is_observed_per_question_within_one_attempt() {
    let suite = suite(&["warm", "cold"]);
    let report = run_v2_authorized(
        TrialEvidence::synthetic(),
        MeasurementCohort::Warm,
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |question| RetrievalObservation {
            result: Ok(bundle(&[])),
            cohort: if question.id == "warm" {
                MeasurementCohort::Warm
            } else {
                MeasurementCohort::Cold
            },
        },
        |_| Ok(()),
    )
    .unwrap();

    assert_eq!(report.questions[0].cohort, Some(MeasurementCohort::Warm));
    assert_eq!(report.questions[1].cohort, Some(MeasurementCohort::Cold));
    let serialized = serde_json::to_string(&report).unwrap();
    assert_eq!(Report::from_str(&serialized).unwrap(), report);
}

#[test]
fn v2_writer_refuses_reader_invalid_headers_before_any_observation() {
    let mut headers = Vec::new();
    let mut header = header_v2();
    header.run_id.clear();
    headers.push(header);
    let mut header = header_v2();
    header.candidate_id.clear();
    headers.push(header);
    let mut header = header_v2();
    header.card_id.clear();
    headers.push(header);
    let mut header = header_v2();
    header.header.suite.clear();
    headers.push(header);
    let mut header = header_v2();
    header.planned_repetitions = 0;
    headers.push(header);
    let mut header = header_v2();
    header
        .header
        .profiles
        .insert(String::new(), "value".to_owned());
    headers.push(header);
    let mut header = header_v2();
    header.warm_up = true;
    header.repetition = 1;
    headers.push(header);
    let mut header = header_v2();
    header.repetition = 0;
    headers.push(header);
    let mut header = header_v2();
    header.retry_of = Some(0);
    headers.push(header);
    let mut header = header_v2();
    header.retry_of = Some(header.attempt);
    headers.push(header);
    let mut header = header_v2();
    header.warm_up = true;
    header.repetition = 0;
    header.retry_of = Some(1);
    headers.push(header);

    for header in headers {
        let observations = Cell::new(0);
        let retrievals = Cell::new(0);
        let result = run_v2_observed(
            header,
            &suite(&["question"]),
            |_| Ok::<_, &str>(None),
            |_| {
                retrievals.set(retrievals.get() + 1);
                Ok(bundle(&[]))
            },
            |_| {
                observations.set(observations.get() + 1);
                Ok(())
            },
        );
        assert!(matches!(result, Err(RunError::InvalidV2Header { .. })));
        assert_eq!(observations.get(), 0);
        assert_eq!(retrievals.get(), 0);
    }
}

#[test]
fn v2_round_trips_and_v1_remains_strict_with_unknown_latency_cohort() {
    let suite = suite(&["empty"]);
    let report = run_v2(
        header_v2(),
        &suite,
        |_| Ok::<_, &str>(None),
        |_| Ok(bundle(&[])),
    )
    .unwrap();
    let bytes = serde_json::to_string(&report).unwrap();
    assert_eq!(Report::from_str(&bytes).unwrap(), report);

    let legacy = report_of(vec![]);
    let legacy_bytes = serde_json::to_string(&legacy).unwrap();
    assert_eq!(Report::from_str(&legacy_bytes).unwrap(), legacy);
    let mut v2_as_v1: Value = serde_json::from_str(&bytes).unwrap();
    v2_as_v1["schema"] = Value::String("maestro-eval-report/1".to_owned());
    assert!(Report::from_str(&v2_as_v1.to_string()).is_err());
}

/// Returns retrieval outcomes that exercise failure retention and continuation.
fn answer_mixed(question: &Question, retrieved: &mut Vec<String>) -> Result<Bundle, &'static str> {
    retrieved.push(question.id.clone());
    match question.id.as_str() {
        "failed" => Err("timeout"),
        "empty" => Ok(bundle(&[])),
        "wrong-generation" => {
            let mut result = bundle(&[]);
            result.generation += 1;
            Ok(result)
        }
        other => panic!("unexpected question {other}"),
    }
}

/// Checks that failures remain rows and do not stop later retrievals.
fn assert_failed_items(report: &Report) {
    assert!(matches!(
        report.questions[0].status,
        Some(ItemStatus::Failed { ref kind, ref reason })
            if kind == "retrieval" && reason == "timeout"
    ));
    assert_eq!(report.questions[1].status, Some(ItemStatus::Succeeded));
    assert!(matches!(
        report.questions[2].status,
        Some(ItemStatus::Failed { ref kind, .. }) if kind == "wrong_generation"
    ));
}

/// Compares floating-point metric output within one machine epsilon.
pub(super) fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < f64::EPSILON,
        "actual {actual:?} != expected {expected:?}"
    );
}

pub(super) fn header_v2() -> HeaderV2 {
    HeaderV2 {
        header: Header {
            suite: "synthetic".to_owned(),
            collection: COLLECTION.to_owned(),
            generation: GENERATION,
            profiles: BTreeMap::new(),
            seed: 17,
        },
        run_id: "synthetic-run".to_owned(),
        candidate_id: "candidate-a".to_owned(),
        card_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
        card_digest: Digest::of(b"candidate card"),
        manifest_digest: Digest::of(b"frozen manifest"),
        planned_repetitions: 1,
        planned_warm_ups: 0,
        corpus_digest: Digest::of(b"frozen corpus"),
        input_digest: Digest::of(b"frozen inputs"),
        attempt: 1,
        repetition: 1,
        retry_of: None,
        attempt_seed: 23,
        route: "dense".to_owned(),
        warm_up: false,
        cross_lingual_questions: BTreeSet::new(),
    }
}

pub(super) fn suite(ids: &[&str]) -> Suite {
    ids.iter()
        .map(|id| {
            serde_json::json!({
                "schema": "maestro-suite/1",
                "id": id,
                "language": "en",
                "question": format!("question {id}"),
                "answerable": false,
                "expected": [],
            })
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .parse()
        .unwrap()
}

#[test]
fn aggregate_error_displays_its_reason() {
    let error = AggregateError::new("attempt reports differ");
    assert_eq!(error.to_string(), "attempt reports differ");
}

#[test]
fn v1_latency_has_no_warm_claim() {
    let legacy = report_of(vec![judge(
        &question("legacy", false),
        &[],
        &bundle(&[]),
        25,
    )]);
    let bytes = serde_json::to_string(&legacy).unwrap();
    assert_eq!(
        legacy.questions[0].measurement_cohort(),
        MeasurementCohort::Unknown
    );
    assert!(Report::from_str(&bytes).is_ok());
    assert!(
        Report::from_str(&bytes.replace(
            "\"schema\":\"maestro-eval-report/1\"",
            "\"schema\":\"unknown\""
        ))
        .is_err()
    );
}
