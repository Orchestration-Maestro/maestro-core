//! Merging v2 attempt reports without dropping scheduled work.

use super::{
    error::AggregateError,
    metric::{measure_attempts, measure_cohorts, measure_subgroups},
    reports::{ItemStatus, QuestionResult, Report, Schema},
};
use std::collections::{BTreeMap, BTreeSet};

/// Combines route-attempt reports for one candidate and suite.
///
/// # Errors
///
/// Returns an error when reports differ in frozen metadata or schedule, omit
/// planned attempts, repeat a question attempt, or contain invalid retries.
pub fn merge_v2_attempts(reports: &[Report]) -> Result<Report, AggregateError> {
    let Some(first) = reports.first() else {
        return Err(AggregateError::new(
            "at least one v2 attempt report is required",
        ));
    };
    if first.schema != Schema::V2 || first.questions.is_empty() {
        return Err(AggregateError::new(
            "only nonempty v2 attempt reports can be merged",
        ));
    }
    let question_ids: Vec<&str> = first.questions.iter().map(|row| row.id.as_str()).collect();
    let route = first
        .questions
        .first()
        .and_then(|question| question.route.as_deref())
        .ok_or_else(|| AggregateError::new("attempt report has no route"))?;
    let mut merged = first.clone();
    merged.questions.clear();
    let mut seen = BTreeSet::new();
    for report in reports {
        if report.schema != Schema::V2
            || report.questions.is_empty()
            || !same_v2_header(first, report)
            || report
                .questions
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>()
                != question_ids
            || report
                .questions
                .iter()
                .any(|row| row.route.as_deref() != Some(route))
        {
            return Err(AggregateError::new(
                "attempt reports differ in frozen header, route or suite rows",
            ));
        }
        let text = serde_json::to_string(report)
            .map_err(|error| AggregateError::new(error.to_string()))?;
        text.parse::<Report>()
            .map_err(|error| AggregateError::new(error.to_string()))?;
        for row in &report.questions {
            let attempt = row
                .attempt
                .ok_or_else(|| AggregateError::new("attempt row has no attempt number"))?;
            if !seen.insert((row.id.clone(), attempt)) {
                return Err(AggregateError::new("a question/attempt row was repeated"));
            }
        }
        merged.questions.extend(report.questions.iter().cloned());
    }
    validate_merged_schedule(&merged)?;
    merged.integrity_violation = Some(
        merged
            .questions
            .iter()
            .any(QuestionResult::is_integrity_violation),
    );
    merged.degraded_searches = merged.questions.iter().filter(|row| row.degraded).count();
    merged.metrics = measure_attempts(&merged.questions, merged.seed);
    merged.cohorts = Some(measure_cohorts(&merged.questions));
    merged.subgroups = Some(measure_subgroups(&merged.questions, merged.seed));
    let text =
        serde_json::to_string(&merged).map_err(|error| AggregateError::new(error.to_string()))?;
    text.parse::<Report>()
        .map_err(|error| AggregateError::new(error.to_string()))
}

/// Checks that merged attempts cover the frozen schedule and retries follow failure.
fn validate_merged_schedule(report: &Report) -> Result<(), AggregateError> {
    let repetitions = report
        .planned_repetitions
        .ok_or_else(|| AggregateError::new("v2 report has no repetition plan"))?;
    let warm_ups = report
        .planned_warm_ups
        .ok_or_else(|| AggregateError::new("v2 report has no warm-up plan"))?;
    if repetitions == 0 {
        return Err(AggregateError::new("repetition plan must be positive"));
    }
    let mut attempts = BTreeMap::new();
    for row in &report.questions {
        let Some(attempt) = row.attempt else {
            return Err(AggregateError::new("attempt row has no attempt number"));
        };
        let metadata = (row.repetition, row.seed, row.retry_of, row.warm_up);
        if attempts
            .insert(attempt, metadata)
            .is_some_and(|previous| previous != metadata)
        {
            return Err(AggregateError::new(
                "one attempt has inconsistent repetition metadata",
            ));
        }
    }
    if attempts.is_empty()
        || attempts
            .keys()
            .copied()
            .enumerate()
            .any(|(index, attempt)| u32::try_from(index + 1).ok() != Some(attempt))
    {
        return Err(AggregateError::new(
            "attempt numbers must be contiguous from one",
        ));
    }
    let mut scheduled_repetitions = BTreeSet::new();
    let mut scheduled_warm_ups = 0_u32;
    for (&attempt, &(repetition, _, retry_of, warm_up)) in &attempts {
        if retry_of.is_none() {
            if warm_up == Some(true) {
                scheduled_warm_ups = scheduled_warm_ups.saturating_add(1);
            } else if let Some(repetition) = repetition
                && !scheduled_repetitions.insert(repetition)
            {
                return Err(AggregateError::new(
                    "a scored repetition has more than one original attempt",
                ));
            }
        }
        let Some(original) = retry_of else {
            continue;
        };
        let Some((original_repetition, _, original_retry, original_warm_up)) =
            attempts.get(&original)
        else {
            return Err(AggregateError::new("retry has no original attempt"));
        };
        let original_failed = report.questions.iter().any(|row| {
            row.attempt == Some(original)
                && matches!(row.status.as_ref(), Some(ItemStatus::Failed { .. }))
        });
        if original >= attempt
            || original_retry.is_some()
            || *original_warm_up != Some(false)
            || *original_repetition != repetition
            || !original_failed
        {
            return Err(AggregateError::new(
                "retry must follow a failed original scored attempt",
            ));
        }
    }
    let planned_repetitions = usize::try_from(repetitions).ok();
    let repetitions_complete = planned_repetitions == Some(scheduled_repetitions.len())
        && scheduled_repetitions
            .iter()
            .copied()
            .enumerate()
            .all(|(index, repetition)| u32::try_from(index + 1).ok() == Some(repetition));
    if !repetitions_complete {
        return Err(AggregateError::new(
            "scored repetitions must cover the planned range exactly once",
        ));
    }
    if scheduled_warm_ups != warm_ups {
        return Err(AggregateError::new(
            "warm-up attempts do not match the frozen plan",
        ));
    }
    Ok(())
}

/// Checks equality of every frozen header field carried by a v2 report.
fn same_v2_header(left: &Report, right: &Report) -> bool {
    left.suite == right.suite
        && left.suite_digest == right.suite_digest
        && left.collection == right.collection
        && left.generation == right.generation
        && left.profiles == right.profiles
        && left.seed == right.seed
        && left.run_id == right.run_id
        && left.candidate_id == right.candidate_id
        && left.card_id == right.card_id
        && left.card_digest == right.card_digest
        && left.manifest_digest == right.manifest_digest
        && left.planned_repetitions == right.planned_repetitions
        && left.planned_warm_ups == right.planned_warm_ups
        && left.corpus_digest == right.corpus_digest
        && left.input_digest == right.input_digest
        && left.mode == right.mode
}
