//! Strict v2 attempt execution and aggregation.

use super::{
    error::RunError,
    judge::judge,
    metric::{measure_attempts, measure_cohorts, measure_subgroups},
    report_validation::{valid_attempt_metadata, valid_v2_header},
    reports::{
        Expected, HeaderV2, ItemStatus, MeasurementCohort, QuestionResult, Report, Schema,
        TrialMode,
    },
    run::resolve,
};
use crate::{
    prepare::QualificationMode,
    suite::{Language, Question, Suite},
};
use maestro_canonicalization::CanonicalDocument;
use maestro_kernel::evidence::Bundle;
use std::{fmt::Display, time::Instant};

/// Validated provenance for one v2 run; callers cannot set it on a report header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::eval) struct TrialEvidence {
    /// Report provenance minted by the runner boundary.
    mode: TrialMode,
}

impl TrialEvidence {
    /// Provenance for the public synthetic-only runner.
    pub(in crate::eval) const fn synthetic() -> Self {
        Self {
            mode: TrialMode::Synthetic,
        }
    }

    /// Derives report provenance from native tokenizer qualification and the
    /// already-validated manifest authorization.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the following bake-off slice uses this authorization seam"
        )
    )]
    pub(in crate::eval) fn authorized(
        qualification_mode: QualificationMode,
        manifest_mode: TrialMode,
    ) -> Result<Self, String> {
        if manifest_mode == TrialMode::Real && qualification_mode != QualificationMode::Native {
            return Err("real evaluation requires native tokenizer qualification".to_owned());
        }
        Ok(Self {
            mode: manifest_mode,
        })
    }

    /// Provenance written to the output report.
    pub(in crate::eval) const fn mode(self) -> TrialMode {
        self.mode
    }
}

/// One retrieval result and its observed latency cohort.
pub(in crate::eval) struct RetrievalObservation<E> {
    /// Bundle or retrieval failure observed for this question.
    pub result: Result<Bundle, E>,
    /// Measured cohort for this item's retrieval.
    pub cohort: MeasurementCohort,
}

/// One durable receipt boundary around an item attempt.
#[derive(Debug, Clone, Copy)]
pub enum AttemptEvent<'a> {
    /// Recorded before the route or retrieval closure runs.
    Started {
        /// Suite question for this attempt.
        question: &'a Question,
        /// Stable run ID.
        run_id: &'a str,
        /// Frozen manifest candidate label.
        candidate_id: &'a str,
        /// Exact registered embedder card ID.
        card_id: &'a str,
        /// One-based attempt number.
        attempt: u32,
        /// Explicit attempt seed.
        seed: u64,
        /// Independent route.
        route: &'a str,
        /// Declared latency cohort.
        cohort: MeasurementCohort,
        /// Whether this attempt is a warm-up.
        warm_up: bool,
    },
    /// Recorded after a success or failure, before the next question starts.
    Completed {
        /// Suite question for this attempt.
        question: &'a Question,
        /// Full persisted row, including typed outcome and elapsed time.
        result: &'a QuestionResult,
    },
}

/// Runs one synthetic v2 attempt, retaining failures and continuing to the
/// next question. Real reports require the crate-private authorized runner.
/// Frozen suite/corpus verification belongs to the caller's preflight.
///
/// # Errors
///
/// Returns an error for unresolved suite inputs, retrieval failure, or an
/// unexpected collection/generation result.
pub fn run_v2<E: Display>(
    header: HeaderV2,
    suite: &Suite,
    documents: impl FnMut(&str) -> Result<Option<CanonicalDocument>, E>,
    retrieve: impl FnMut(&Question) -> Result<Bundle, E>,
) -> Result<Report, RunError<E>> {
    run_v2_observed(header, suite, documents, retrieve, |_| Ok(()))
}

/// `run_v2`, with append-only observations. Public runs are synthetic-only;
/// a recording error aborts before any subsequent retrieval.
///
/// # Errors
///
/// Returns an error for invalid frozen headers, unresolved suite inputs,
/// retrieval failure, or a recording failure.
pub fn run_v2_observed<E: Display>(
    header: HeaderV2,
    suite: &Suite,
    documents: impl FnMut(&str) -> Result<Option<CanonicalDocument>, E>,
    retrieve: impl FnMut(&Question) -> Result<Bundle, E>,
    observe: impl FnMut(AttemptEvent<'_>) -> Result<(), String>,
) -> Result<Report, RunError<E>> {
    let mut retrieve = retrieve;
    run_v2_with_evidence(
        TrialEvidence::synthetic(),
        MeasurementCohort::Unknown,
        header,
        suite,
        documents,
        |question| RetrievalObservation {
            result: retrieve(question),
            cohort: MeasurementCohort::Unknown,
        },
        observe,
    )
}

/// Runs one report using evidence minted by the validated bake-off boundary.
///
/// # Errors
///
/// Returns an error for invalid frozen headers, unresolved suite inputs,
/// retrieval failure, or a recording failure.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the following bake-off slice uses this authorized runner"
    )
)]
#[expect(
    clippy::too_many_arguments,
    reason = "the authorized runner keeps evidence and callback seams explicit"
)]
pub(in crate::eval) fn run_v2_authorized<E: Display>(
    evidence: TrialEvidence,
    planned_cohort: MeasurementCohort,
    header: HeaderV2,
    suite: &Suite,
    documents: impl FnMut(&str) -> Result<Option<CanonicalDocument>, E>,
    retrieve: impl FnMut(&Question) -> RetrievalObservation<E>,
    observe: impl FnMut(AttemptEvent<'_>) -> Result<(), String>,
) -> Result<Report, RunError<E>> {
    run_v2_with_evidence(
        evidence,
        planned_cohort,
        header,
        suite,
        documents,
        retrieve,
        observe,
    )
}

/// Shared v2 execution after provenance is fixed by the entry point.
#[expect(
    clippy::too_many_arguments,
    reason = "the runner keeps frozen inputs and callback seams explicit"
)]
fn run_v2_with_evidence<E: Display>(
    evidence: TrialEvidence,
    planned_cohort: MeasurementCohort,
    header: HeaderV2,
    suite: &Suite,
    mut documents: impl FnMut(&str) -> Result<Option<CanonicalDocument>, E>,
    mut retrieve: impl FnMut(&Question) -> RetrievalObservation<E>,
    mut observe: impl FnMut(AttemptEvent<'_>) -> Result<(), String>,
) -> Result<Report, RunError<E>> {
    validate_attempt_header(&header, suite, evidence.mode())
        .map_err(|reason| RunError::InvalidV2Header { reason })?;
    let expected = resolve(suite, &mut documents)?;
    let mut questions = Vec::with_capacity(suite.questions.len());
    for (question, expected) in suite.questions.iter().zip(&expected) {
        questions.push(run_attempt(
            &header,
            planned_cohort,
            question,
            expected,
            &mut retrieve,
            &mut observe,
        )?);
    }
    let metrics = measure_attempts(&questions, header.header.seed);
    let cohorts = measure_cohorts(&questions);
    let subgroups = measure_subgroups(&questions, header.header.seed);
    let degraded_searches = questions.iter().filter(|result| result.degraded).count();
    let integrity_violation = questions.iter().any(QuestionResult::is_integrity_violation);
    let report = Report {
        schema: Schema::V2,
        suite: header.header.suite,
        suite_digest: suite.digest.clone(),
        collection: header.header.collection,
        generation: header.header.generation,
        profiles: header.header.profiles,
        seed: header.header.seed,
        questions,
        degraded_searches,
        metrics,
        run_id: Some(header.run_id),
        candidate_id: Some(header.candidate_id),
        card_id: Some(header.card_id),
        card_digest: Some(header.card_digest),
        manifest_digest: Some(header.manifest_digest),
        planned_repetitions: Some(header.planned_repetitions),
        planned_warm_ups: Some(header.planned_warm_ups),
        corpus_digest: Some(header.corpus_digest),
        input_digest: Some(header.input_digest),
        mode: Some(evidence.mode()),
        integrity_violation: Some(integrity_violation),
        cohorts: Some(cohorts),
        subgroups: Some(subgroups),
    };
    let serialized = serde_json::to_string(&report).map_err(|error| RunError::InvalidV2Report {
        reason: error.to_string(),
    })?;
    serialized
        .parse::<Report>()
        .map_err(|error| RunError::InvalidV2Report {
            reason: error.to_string(),
        })
}

/// Checks every frozen field and item rule before the first callback.
fn validate_attempt_header(
    header: &HeaderV2,
    suite: &Suite,
    mode: TrialMode,
) -> Result<(), String> {
    if !valid_v2_header(
        &header.header.suite,
        &header.header.collection,
        &header.header.profiles,
        Some(&header.run_id),
        Some(&header.candidate_id),
        Some(&header.card_id),
        Some(&header.card_digest),
        Some(&header.manifest_digest),
        Some(&header.corpus_digest),
        Some(&header.input_digest),
        Some(mode),
        Some(header.planned_repetitions),
        Some(header.planned_warm_ups),
    ) {
        return Err("frozen header fields must be present and nonblank".to_owned());
    }
    if !valid_attempt_metadata(
        Some(header.attempt),
        Some(header.repetition),
        header.retry_of,
        Some(&header.route),
        Some(header.warm_up),
    ) {
        return Err("attempt, repetition, route and retry metadata are inconsistent".to_owned());
    }
    if header
        .cross_lingual_questions
        .iter()
        .any(|id| !suite.questions.iter().any(|question| question.id == *id))
    {
        return Err("cross-lingual labels name a question outside the frozen suite".to_owned());
    }
    Ok(())
}

/// Runs, scores and durably closes one question attempt.
#[expect(
    clippy::too_many_arguments,
    reason = "the attempt needs its frozen context and two independent callbacks"
)]
fn run_attempt<E: Display>(
    header: &HeaderV2,
    planned_cohort: MeasurementCohort,
    question: &Question,
    expected: &[Expected],
    retrieve: &mut impl FnMut(&Question) -> RetrievalObservation<E>,
    observe: &mut impl FnMut(AttemptEvent<'_>) -> Result<(), String>,
) -> Result<QuestionResult, RunError<E>> {
    observe(AttemptEvent::Started {
        question,
        run_id: &header.run_id,
        candidate_id: &header.candidate_id,
        card_id: &header.card_id,
        attempt: header.attempt,
        seed: header.attempt_seed,
        route: &header.route,
        cohort: planned_cohort,
        warm_up: header.warm_up,
    })
    .map_err(|reason| RunError::Recording {
        question: question.id.clone(),
        reason,
    })?;
    let start = Instant::now();
    let observation = retrieve(question);
    let elapsed_us = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX);
    let latency_us = u32::try_from(elapsed_us).unwrap_or(u32::MAX);
    let mut result = match observation.result {
        Err(error) => {
            let mut result = failed_result(question, expected, latency_us);
            result.status = Some(ItemStatus::Failed {
                kind: "retrieval".to_owned(),
                reason: error.to_string(),
            });
            result
        }
        Ok(bundle)
            if bundle.collection != header.header.collection
                || bundle.generation != header.header.generation =>
        {
            let mut result = failed_result(question, expected, latency_us);
            result.status = Some(ItemStatus::Failed {
                kind: if bundle.collection == header.header.collection {
                    "wrong_generation".to_owned()
                } else {
                    "wrong_collection".to_owned()
                },
                reason: format!(
                    "received generation {} of {}, expected generation {} of {}",
                    bundle.generation,
                    bundle.collection,
                    header.header.generation,
                    header.header.collection
                ),
            });
            result
        }
        Ok(bundle) => {
            let mut result = judge(question, expected, &bundle, latency_us);
            result.status = Some(ItemStatus::Succeeded);
            result
        }
    };
    result.attempt = Some(header.attempt);
    result.repetition = Some(header.repetition);
    result.retry_of = header.retry_of;
    result.seed = Some(header.attempt_seed);
    result.route = Some(header.route.clone());
    result.elapsed_us = Some(elapsed_us);
    result.cohort = Some(observation.cohort);
    result.warm_up = Some(header.warm_up);
    result.language = Some(
        match question.language {
            Language::Fr => "fr",
            Language::En => "en",
        }
        .to_owned(),
    );
    result.cross_lingual = Some(header.cross_lingual_questions.contains(&question.id));
    observe(AttemptEvent::Completed {
        question,
        result: &result,
    })
    .map_err(|reason| RunError::Recording {
        question: question.id.clone(),
        reason,
    })?;
    Ok(result)
}

/// An attempted but failed item has no passages or expected ranks and cannot
/// earn abstention credit.
fn failed_result(question: &Question, expected: &[Expected], latency_us: u32) -> QuestionResult {
    QuestionResult {
        id: question.id.clone(),
        answerable: question.answerable,
        expected: expected.to_vec(),
        passages: 0,
        latency_us,
        degraded: true,
        failures: Vec::new(),
        attempt: None,
        repetition: None,
        retry_of: None,
        seed: None,
        route: None,
        status: None,
        elapsed_us: None,
        cohort: None,
        warm_up: None,
        language: None,
        cross_lingual: None,
    }
}
