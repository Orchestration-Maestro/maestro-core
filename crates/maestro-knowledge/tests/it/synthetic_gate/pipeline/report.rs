//! Writes public-only reports, provenance and summaries for a gate run.

use super::super::failure::Failure;
use super::contract::Output;
use maestro_knowledge::eval;
use serde::Serialize;
use serde_json::json;
use std::{env, fs::OpenOptions, io::Write as _, path::Path};

pub(super) fn write_output(directory: &Path, output: &Output) -> Result<(), Failure> {
    write_json(directory, "report.json", &output.report)?;
    write_json(directory, "rankings.json", &output.rankings)?;
    write_json(directory, "provenance.json", &output.provenance)
}

pub(super) fn write_failure(directory: &Path, failure: &Failure) {
    let record = json!({
        "schema": "maestro-synthetic-run-failure/1",
        "stage": failure.stage,
        "item": failure.item,
        "completed": failure.completed,
    });
    drop(write_json(directory, "failure.json", &record));
}

fn write_json(directory: &Path, name: &str, value: &impl Serialize) -> Result<(), Failure> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| Failure::from_error("report-serialization", error))?;
    let path = directory.join(name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| Failure::from_error("report-write", error))?;
    file.write_all(&bytes)
        .map_err(|error| Failure::from_error("report-write", error))
}

pub(in crate::synthetic_gate) fn summary(output: &Output) -> String {
    let metric = |value: Option<eval::Estimate>| value.map(|estimate| estimate.value);
    let baseline = match &output.baseline_digest {
        Some(digest) => format!("Baseline: `{digest}`; all six quality metrics passed.\\n\\n"),
        None => {
            "Baseline: no baseline on the trusted base; this run passed without comparison.\\n\\n"
                .to_owned()
        }
    };
    format!(
        "## Public synthetic retrieval gate\n\n\
         Backend: `{}`; questions: {}; errors: 0; degraded: {}.\n\n\
         Recall@5: {:?}; Recall@10: {:?}; MRR@10: {:?}; nDCG@10: {:?}; \
         no-answer accuracy: {:?}; false abstentions: {:?}.\n\n\
         {}",
        output.provenance.backend,
        output.report.questions.len(),
        output.report.degraded_searches,
        metric(output.report.metrics.recall_at_5),
        metric(output.report.metrics.recall_at_10),
        metric(output.report.metrics.mrr_at_10),
        metric(output.report.metrics.ndcg_at_10),
        metric(output.report.metrics.no_answer_accuracy),
        metric(output.report.metrics.false_abstentions),
        baseline,
    )
}

pub(in crate::synthetic_gate) fn append_summary(text: &str) {
    let Some(path) = env::var_os("GITHUB_STEP_SUMMARY") else {
        return;
    };
    if let Ok(mut file) = OpenOptions::new().append(true).create(true).open(path) {
        drop(file.write_all(text.as_bytes()));
    }
}

pub(super) fn metrics_complete(report: &eval::Report) -> bool {
    [
        report.metrics.recall_at_5,
        report.metrics.recall_at_10,
        report.metrics.mrr_at_10,
        report.metrics.ndcg_at_10,
        report.metrics.no_answer_accuracy,
        report.metrics.false_abstentions,
    ]
    .into_iter()
    .flatten()
    .all(|metric| metric.value.is_finite() && (0.0..=1.0).contains(&metric.value))
        && report.metrics.recall_at_5.is_some()
        && report.metrics.recall_at_10.is_some()
        && report.metrics.mrr_at_10.is_some()
        && report.metrics.ndcg_at_10.is_some()
        && report.metrics.no_answer_accuracy.is_some()
        && report.metrics.false_abstentions.is_some()
}
