//! Synthetic-gate policy checks, built from strict T021 reports.

use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::FakeModels,
};
use maestro_knowledge::{eval::Report, prepare::RouterTokenizer};
use serde_json::json;
use std::{env, error::Error, fs};

use super::{
    comparison::require_no_regression, models::SyntheticModels, pipeline, support::TestDirectory,
};

#[test]
fn synthetic_inference_requires_unchanged_native_tokenizer_canaries() -> Result<(), Box<dyn Error>>
{
    let root = TestDirectory::new("maestro-synthetic-card")?;
    let store = Store::new(&root.path);
    let card = SyntheticModels::card(&store).unwrap();
    assert!(RouterTokenizer::qualify(FakeModels, card.clone()).is_err());
    let tokenizer =
        RouterTokenizer::qualify(SyntheticModels::new().unwrap(), card.clone()).unwrap();
    assert!(
        RouterTokenizer::qualify(SyntheticModels::new().unwrap().changed_canary(), card).is_err()
    );
    drop(tokenizer);
    drop(store);
    Ok(())
}

#[test]
fn pipeline_uses_fake_qdrant_for_import_quality_prepare_publish_and_evaluation() {
    let output = pipeline::run_fake().unwrap();

    assert_pipeline_counts(&output);
    let public = public_output(&output).unwrap();
    assert!(!public.contains(env!("CARGO_MANIFEST_DIR")));
    assert!(!public.contains(env::temp_dir().to_string_lossy().as_ref()));
    if let Some(home) = env::var_os("HOME") {
        assert!(!public.contains(home.to_string_lossy().as_ref()));
    }
    assert!(!public.contains("MAESTRO_ROUTER_URL"));
    assert!(!public.contains("PRIVATE"));
}

#[test]
fn a_dead_qdrant_endpoint_fails_at_the_first_dense_query() {
    let failure = pipeline::run_fake_with_dead_endpoint().unwrap_err();
    assert_eq!(
        failure,
        super::failure::Failure::question("dense", "backup-retention-days", 0)
    );
}

#[test]
fn baseline_files_are_strict_and_quality_regressions_fail() -> Result<(), Box<dyn Error>> {
    let baseline_report = report([1.0, 1.0, 1.0, 1.0, 1.0, 0.0], 1).unwrap();
    let worse = report([0.0, 1.0, 1.0, 1.0, 1.0, 0.0], 2).unwrap();
    let directory = TestDirectory::new("maestro-synthetic-baseline")?;
    let path = directory.path.join("baseline.json");
    let bytes = serde_json::to_vec(&baseline_report).unwrap();
    fs::write(&path, &bytes).unwrap();

    let baseline = pipeline::baseline::load(&path).unwrap();
    assert_eq!(baseline.digest, Digest::of(&bytes).as_str());
    assert_eq!(
        pipeline::baseline::compare(&baseline, &worse)
            .unwrap_err()
            .stage,
        "baseline-regression"
    );

    let mut identity_mismatch = baseline_report.clone();
    identity_mismatch
        .profiles
        .insert("foreign".to_owned(), "profile".to_owned());
    assert_eq!(
        pipeline::baseline::compare(&baseline, &identity_mismatch)
            .unwrap_err()
            .stage,
        "baseline-identity"
    );

    let mut invalid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    invalid["unexpected"] = serde_json::json!(true);
    fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
    assert_eq!(
        pipeline::baseline::load(&path).unwrap_err().stage,
        "baseline-format"
    );
    Ok(())
}

fn assert_pipeline_counts(output: &pipeline::Output) {
    assert_import_counts(output);
    assert_preparation_counts(output);
    assert_evaluation_counts(output);
    assert_metric_presence(output);
}

fn assert_import_counts(output: &pipeline::Output) {
    assert_eq!(output.counts.imported, 28);
    assert_eq!(output.counts.quality_decided, 28);
    assert_eq!(output.counts.accepted, 28);
}

fn assert_preparation_counts(output: &pipeline::Output) {
    assert_eq!(output.counts.eligible, 28);
    assert_eq!(output.counts.prepared, 27);
    assert_eq!(output.counts.duplicates, 1);
    assert!(output.counts.chunks > 0);
    assert_eq!(output.counts.published_points, output.counts.chunks);
}

fn assert_evaluation_counts(output: &pipeline::Output) {
    assert_eq!(output.report.questions.len(), 56);
    assert_eq!(output.rankings.len(), 56);
    assert_eq!(output.counts.dense_queries, 56);
    assert_eq!(output.counts.completed_questions, 56);
    assert_eq!(output.report.degraded_searches, 0);
    assert!(
        output
            .report
            .questions
            .iter()
            .all(|question| !question.degraded)
    );
    assert_eq!(output.baseline_digest, None);
    assert_eq!(
        output.model_identity,
        "fake-models/1 (synthetic protocol simulation)"
    );
}

fn assert_metric_presence(output: &pipeline::Output) {
    assert!(output.report.metrics.recall_at_5.is_some());
    assert!(output.report.metrics.recall_at_10.is_some());
    assert!(output.report.metrics.mrr_at_10.is_some());
    assert!(output.report.metrics.ndcg_at_10.is_some());
    assert!(output.report.metrics.no_answer_accuracy.is_some());
    assert!(output.report.metrics.false_abstentions.is_some());
}

fn public_output(output: &pipeline::Output) -> Result<String, serde_json::Error> {
    Ok(format!(
        "{}{}{}",
        serde_json::to_string(&output.report)?,
        serde_json::to_string(&output.rankings)?,
        serde_json::to_string(&output.provenance)?
    ))
}

#[test]
fn a_lower_recall_value_is_a_deterministic_regression() -> Result<(), serde_json::Error> {
    let baseline = report([0.75, 0.9, 0.8, 0.85, 0.5, 0.1], 1)?;
    let candidate = report(
        [
            f64::from_bits(0.75_f64.to_bits() - 1),
            0.9,
            0.8,
            0.85,
            0.5,
            0.1,
        ],
        2,
    )?;
    assert!(require_no_regression(&baseline, &candidate).is_err());
    Ok(())
}

#[test]
fn exact_equality_improvements_and_new_generation_pass() -> Result<(), serde_json::Error> {
    let baseline = report([0.75, 0.9, 0.8, 0.85, 0.5, 0.1], 1)?;
    let equal = report([0.75, 0.9, 0.8, 0.85, 0.5, 0.1], 2)?;
    let improved = report([0.8, 0.95, 0.85, 0.9, 0.6, 0.05], 2)?;
    assert_eq!(require_no_regression(&baseline, &equal), Ok(()));
    assert_eq!(require_no_regression(&baseline, &improved), Ok(()));
    Ok(())
}

#[test]
fn every_quality_metric_uses_its_exact_direction() -> Result<(), serde_json::Error> {
    let before: [f64; 6] = [0.75, 0.9, 0.8, 0.85, 0.5, 0.1];
    let names = [
        "recall@5",
        "recall@10",
        "mrr@10",
        "ndcg@10",
        "no_answer_accuracy",
        "false_abstentions",
    ];
    let baseline = report(before, 1)?;
    for index in 0..before.len() {
        let mut after = before;
        after[index] = if index == 5 {
            f64::from_bits(after[index].to_bits() + 1)
        } else {
            f64::from_bits(after[index].to_bits() - 1)
        };
        let candidate = report(after, 2)?;
        assert_eq!(
            require_no_regression(&baseline, &candidate),
            Err(super::comparison::Refusal::Regression(names[index]))
        );
    }
    Ok(())
}

#[test]
fn missing_nonfinite_or_out_of_range_metric_values_refuse() -> Result<(), serde_json::Error> {
    let baseline = report([0.75, 0.9, 0.8, 0.85, 0.5, 0.1], 1)?;
    let mut missing = report([0.75, 0.9, 0.8, 0.85, 0.5, 0.1], 2)?;
    missing.metrics.recall_at_10 = None;
    assert_eq!(
        require_no_regression(&baseline, &missing),
        Err(super::comparison::Refusal::MissingMetric("recall@10"))
    );
    let mut not_finite = report([0.75, 0.9, 0.8, 0.85, 0.5, 0.1], 2)?;
    not_finite.metrics.mrr_at_10.as_mut().unwrap().value = f64::NAN;
    assert_eq!(
        require_no_regression(&baseline, &not_finite),
        Err(super::comparison::Refusal::InvalidMetric("mrr@10"))
    );
    let mut out_of_range = report([0.75, 0.9, 0.8, 0.85, 0.5, 0.1], 2)?;
    out_of_range
        .metrics
        .no_answer_accuracy
        .as_mut()
        .unwrap()
        .high = 1.1;
    assert_eq!(
        require_no_regression(&baseline, &out_of_range),
        Err(super::comparison::Refusal::InvalidMetric(
            "no_answer_accuracy"
        ))
    );
    Ok(())
}

#[test]
fn identity_and_complete_question_set_are_required() -> Result<(), serde_json::Error> {
    let baseline = report([0.75, 0.9, 0.8, 0.85, 0.5, 0.1], 1)?;
    let mut wrong_profile = report([0.75, 0.9, 0.8, 0.85, 0.5, 0.1], 2)?;
    wrong_profile
        .profiles
        .insert("corpus_manifest".to_owned(), "changed/1".to_owned());
    assert_eq!(
        require_no_regression(&baseline, &wrong_profile),
        Err(super::comparison::Refusal::Identity)
    );
    let mut missing = report([0.75, 0.9, 0.8, 0.85, 0.5, 0.1], 2)?;
    missing.questions.pop();
    assert_eq!(
        require_no_regression(&baseline, &missing),
        Err(super::comparison::Refusal::Incomplete)
    );
    let mut duplicate = report([0.75, 0.9, 0.8, 0.85, 0.5, 0.1], 2)?;
    duplicate.questions[55].id = duplicate.questions[0].id.clone();
    assert_eq!(
        require_no_regression(&baseline, &duplicate),
        Err(super::comparison::Refusal::Incomplete)
    );
    let mut substituted = report([0.75, 0.9, 0.8, 0.85, 0.5, 0.1], 2)?;
    for question in &mut substituted.questions {
        question.answerable = false;
        question.expected.clear();
    }
    assert_eq!(
        require_no_regression(&baseline, &substituted),
        Err(super::comparison::Refusal::Incomplete)
    );
    Ok(())
}

#[test]
fn degraded_reports_are_not_quality_baselines_or_candidates() -> Result<(), serde_json::Error> {
    let baseline = report([0.75, 0.9, 0.8, 0.85, 0.5, 0.1], 1)?;
    let mut candidate = report([0.75, 0.9, 0.8, 0.85, 0.5, 0.1], 2)?;
    candidate.questions[0].degraded = true;
    assert_eq!(
        require_no_regression(&baseline, &candidate),
        Err(super::comparison::Refusal::Degraded)
    );
    Ok(())
}

fn report(values: [f64; 6], generation: i64) -> Result<Report, serde_json::Error> {
    let digest = Digest::of(b"synthetic-suite");
    let questions = (0..56)
        .map(|number| {
            let answerable = number < 48;
            json!({
                "id": format!("q-{number:02}"),
                "answerable": answerable,
                "expected": if answerable {
                    json!([{"document_id":"doc", "section_id":"section", "rank":1}])
                } else {
                    json!([])
                },
                "passages": usize::from(answerable),
                "latency_us": 100,
                "degraded": false,
                "failures": []
            })
        })
        .collect::<Vec<_>>();
    serde_json::from_value(json!({
        "schema": "maestro-eval-report/1",
        "suite": "synthetic",
        "suite_digest": digest.as_str(),
        "collection": "synthetic",
        "generation": generation,
        "profiles": {"pipeline": "synthetic/1"},
        "seed": 17,
        "questions": questions,
        "degraded_searches": 0,
        "metrics": {
            "recall_at_5": estimate(values[0]),
            "recall_at_10": estimate(values[1]),
            "mrr_at_10": estimate(values[2]),
            "ndcg_at_10": estimate(values[3]),
            "no_answer_accuracy": estimate(values[4]),
            "false_abstentions": estimate(values[5])
        }
    }))
}

fn estimate(value: f64) -> serde_json::Value {
    json!({"value": value, "low": value, "high": value})
}
