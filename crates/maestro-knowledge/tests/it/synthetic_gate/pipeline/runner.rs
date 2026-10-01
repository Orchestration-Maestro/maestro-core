//! Runs the test adapter against fake Qdrant or the pinned real service.

use super::super::{failure::Failure, qdrant};
use super::{
    baseline,
    contract::{self, Output},
    driver::{execute, execute_real, execute_with_search_qdrant},
    report::{write_failure, write_output},
};
use crate::qdrant_projection;
use maestro_knowledge::{eval::Estimate, index::Qdrant};
use std::{env, fs, io, path::Path};
use tokio::runtime::{Builder, Runtime};

/// Runs one independent scratch kernel against the in-memory fake Qdrant.
pub(in crate::synthetic_gate) fn run_fake() -> Result<Output, Failure> {
    let runtime = runtime().map_err(|error| Failure::from_error("runtime", error))?;
    let url = runtime.block_on(async { qdrant_projection::synthetic_fake_qdrant_url() });
    let qdrant = Qdrant::new(&url).map_err(|error| Failure::from_error("qdrant-client", error))?;
    let mut generations = Vec::new();
    let result = execute(
        &runtime,
        &qdrant,
        "fake-qdrant-dot-product/1",
        &mut generations,
    );
    clean_after_run(&runtime, &url, &generations, result, false)
}

/// Runs against a working publisher but a dead search endpoint, for failure accounting.
pub(in crate::synthetic_gate) fn run_fake_with_dead_endpoint() -> Result<Output, Failure> {
    let runtime = runtime().map_err(|error| Failure::from_error("runtime", error))?;
    let url = runtime.block_on(async { qdrant_projection::synthetic_fake_qdrant_url() });
    let publisher =
        Qdrant::new(&url).map_err(|error| Failure::from_error("qdrant-client", error))?;
    let dead = Qdrant::new("http://127.0.0.1:1")
        .map_err(|error| Failure::from_error("qdrant-client", error))?;
    let mut generations = Vec::new();
    let result = execute_with_search_qdrant(
        &runtime,
        &publisher,
        &dead,
        "fake-qdrant-dot-product/1",
        &mut generations,
    );
    clean_after_run(&runtime, &url, &generations, result, false)
}

/// Mandatory real-Qdrant CI entry point; a missing or foreign service fails, never skips.
pub(in crate::synthetic_gate) fn run_real_qdrant_baseline(
    report_dir: &Path,
    url: Option<&str>,
) -> Result<(Output, Output), Failure> {
    fs::create_dir_all(report_dir)
        .map_err(|error| Failure::from_error("report-directory", error))?;
    let result = run_real_qdrant(report_dir, url);
    result.map_err(|failure| record_failure(report_dir, failure))
}

fn run_real_qdrant(report_dir: &Path, url: Option<&str>) -> Result<(Output, Output), Failure> {
    let Some(url) = url else {
        return Err(Failure::new("qdrant-url"));
    };
    if url != qdrant::REAL_URL {
        return Err(Failure::new("qdrant-url-ownership"));
    }
    let runtime = runtime().map_err(|error| Failure::from_error("runtime", error))?;
    let version =
        runtime
            .block_on(qdrant::require_real_qdrant(url))
            .map_err(|error| match &error {
                qdrant::PreflightError::NotEmpty => Failure::from_error("qdrant-not-empty", error),
                _ => Failure::from_error("qdrant-preflight", error),
            })?;
    let qdrant = Qdrant::new(url).map_err(|error| Failure::from_error("qdrant-client", error))?;
    let backend = format!("qdrant-server/{version}");

    let mut first_generations = Vec::new();
    let first_result = execute_real(&runtime, &qdrant, &backend, url, &mut first_generations);
    let first = clean_after_run(&runtime, url, &first_generations, first_result, true)?;

    let mut second_generations = Vec::new();
    let second_result = execute_real(&runtime, &qdrant, &backend, url, &mut second_generations);
    let mut second = clean_after_run(&runtime, url, &second_generations, second_result, true)?;
    if !contract::same_run_identity(&first, &second) {
        write_output(report_dir, &second)?;
        return Err(Failure::new("repeated-run-mismatch"));
    }

    let baseline = match env::var_os("MAESTRO_SYNTHETIC_BASELINE") {
        Some(path) => match baseline::load(Path::new(&path)) {
            Ok(baseline) => Some(baseline),
            Err(failure) => {
                write_output(report_dir, &second)?;
                return Err(failure);
            }
        },
        None => None,
    };
    if let Some(baseline) = &baseline {
        let digest = baseline.digest.clone();
        second.baseline_digest = Some(digest.clone());
        second.provenance.baseline_digest = Some(digest);
    }
    force_worse_candidate_for_ci_test(&mut second)?;
    write_output(report_dir, &second)?;
    if let Some(baseline) = baseline {
        baseline::compare(&baseline, &second.report)?;
    }
    Ok((first, second))
}

#[cfg(test)]
fn force_worse_candidate_for_ci_test(output: &mut Output) -> Result<(), Failure> {
    if env::var_os("MAESTRO_SYNTHETIC_TEST_BAD_CANDIDATE").is_some() {
        let Some(metric) = output.report.metrics.recall_at_5.as_mut() else {
            return Err(Failure::new("test-candidate-metric"));
        };
        *metric = Estimate {
            low: 0.0,
            value: 0.0,
            high: 0.0,
        };
    }
    Ok(())
}

#[cfg(not(test))]
fn force_worse_candidate_for_ci_test(_: &mut Output) -> Result<(), Failure> {
    Ok(())
}

fn runtime() -> Result<Runtime, io::Error> {
    Builder::new_current_thread().enable_all().build()
}

fn clean_after_run(
    runtime: &Runtime,
    url: &str,
    generations: &[i64],
    result: Result<Output, Failure>,
    verify_empty: bool,
) -> Result<Output, Failure> {
    let cleanup = runtime.block_on(qdrant::cleanup(url, generations));
    let empty = verify_empty.then(|| runtime.block_on(qdrant::require_empty(url)));
    let cleanup_failure = match cleanup {
        Err(error) => Some(Failure::from_error("qdrant-cleanup", error)),
        Ok(()) => empty.and_then(Result::err).map(|error| match error {
            qdrant::PreflightError::NotEmpty => Failure::from_error("qdrant-not-empty", error),
            _ => Failure::from_error("qdrant-empty-check", error),
        }),
    };
    match (result, cleanup_failure) {
        (Err(original), Some(cleanup)) => {
            eprintln!("secondary cleanup failure: {cleanup}");
            Err(original)
        }
        (Err(original), None) => Err(original),
        (Ok(_), Some(cleanup)) => Err(cleanup),
        (Ok(output), None) => Ok(output),
    }
}

fn record_failure(report_dir: &Path, failure: Failure) -> Failure {
    write_failure(report_dir, &failure);
    failure
}
