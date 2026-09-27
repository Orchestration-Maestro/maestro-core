//! Comparing runs question by question, with repeated v2 attempts kept in
//! their question clusters.

use super::{
    bootstrap::estimates,
    error::CompareError,
    metric::{metrics, values},
    reports::{Metrics, QuestionResult, Report, Schema},
};
use std::collections::{BTreeMap, BTreeSet};

/// Two runs compared: for each metric, the candidate's value minus the
/// baseline's, with its interval.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Comparison {
    /// The seed the paired resamples were drawn with.
    pub seed: u64,
    /// How many distinct question clusters the two runs share.
    pub questions: usize,
    /// Attempt rows pooled from the baseline report.
    pub baseline_attempts: usize,
    /// Attempt rows pooled from the candidate report.
    pub candidate_attempts: usize,
    /// For each metric, the candidate's value minus the baseline's, over the
    /// questions the runs share, with the 95 % interval of that difference;
    /// absent when a run's metric covers no question.
    pub differences: Metrics,
}

/// The paired comparison of `candidate` with `baseline`.
///
/// V1 reports pair one row per question. V2 checks scheduled repetitions and
/// seeds, then pools each side's retries within the same bootstrapped question
/// clusters; no retry is selected as the best result.
///
/// # Errors
///
/// Returns an error when collection, suite, generation evidence, mode, route,
/// attempt schedule or question labels cannot be paired.
pub fn compare(
    baseline: &Report,
    candidate: &Report,
    seed: u64,
) -> Result<Comparison, CompareError> {
    validate_metadata(baseline, candidate)?;
    let clusters = match baseline.schema {
        Schema::V1 => pair_v1(&baseline.questions, &candidate.questions)?,
        Schema::V2 => pair_v2(&baseline.questions, &candidate.questions)?,
    };
    let answerable: Vec<bool> = clusters
        .iter()
        .map(|(baseline, _)| baseline.first().is_some_and(|row| row.answerable))
        .collect();
    let baseline_attempts = clusters.iter().map(|(rows, _)| rows.len()).sum();
    let candidate_attempts = clusters.iter().map(|(_, rows)| rows.len()).sum();
    let found = estimates(&answerable, seed, |sample| {
        let baseline_rows: Vec<&QuestionResult> = sample
            .iter()
            .filter_map(|&index| clusters.get(index))
            .flat_map(|(rows, _)| rows.iter().copied())
            .collect();
        let candidate_rows: Vec<&QuestionResult> = sample
            .iter()
            .filter_map(|&index| clusters.get(index))
            .flat_map(|(_, rows)| rows.iter().copied())
            .collect();
        let before = values(&baseline_rows);
        values(&candidate_rows)
            .into_iter()
            .filter_map(|(metric, value)| Some((metric, value - before.get(&metric)?)))
            .collect()
    });
    Ok(Comparison {
        seed,
        questions: clusters.len(),
        baseline_attempts,
        candidate_attempts,
        differences: metrics(&found),
    })
}

/// Attempts for one question in the baseline and candidate reports.
type PairedCluster<'a> = (Vec<&'a QuestionResult>, Vec<&'a QuestionResult>);

/// Checks run-level metadata required for a meaningful comparison.
fn validate_metadata(baseline: &Report, candidate: &Report) -> Result<(), CompareError> {
    if baseline.collection != candidate.collection {
        return Err(CompareError::Collection {
            baseline: baseline.collection.clone(),
            candidate: candidate.collection.clone(),
        });
    }
    if baseline.suite != candidate.suite {
        return Err(CompareError::Suite {
            baseline: baseline.suite.clone(),
            candidate: candidate.suite.clone(),
        });
    }
    if baseline.suite_digest != candidate.suite_digest {
        return Err(CompareError::SuiteDigest {
            suite: baseline.suite.clone(),
            baseline: baseline.suite_digest.clone(),
            candidate: candidate.suite_digest.clone(),
        });
    }
    match (baseline.schema, candidate.schema) {
        (Schema::V1, Schema::V1) => Ok(()),
        (Schema::V2, Schema::V2) => validate_v2_metadata(baseline, candidate),
        _ => Err(CompareError::Version),
    }
}

/// Checks frozen digests, execution mode and route for v2 reports.
fn validate_v2_metadata(baseline: &Report, candidate: &Report) -> Result<(), CompareError> {
    let (Some(baseline_corpus), Some(candidate_corpus)) =
        (&baseline.corpus_digest, &candidate.corpus_digest)
    else {
        return Err(CompareError::Version);
    };
    if baseline_corpus != candidate_corpus {
        return Err(CompareError::CorpusDigest {
            baseline: baseline_corpus.clone(),
            candidate: candidate_corpus.clone(),
        });
    }
    let (Some(baseline_input), Some(candidate_input)) =
        (&baseline.input_digest, &candidate.input_digest)
    else {
        return Err(CompareError::Version);
    };
    if baseline_input != candidate_input {
        return Err(CompareError::InputDigest {
            baseline: baseline_input.clone(),
            candidate: candidate_input.clone(),
        });
    }
    let (Some(baseline_manifest), Some(candidate_manifest)) =
        (&baseline.manifest_digest, &candidate.manifest_digest)
    else {
        return Err(CompareError::Version);
    };
    if baseline_manifest != candidate_manifest {
        return Err(CompareError::ManifestDigest {
            baseline: baseline_manifest.clone(),
            candidate: candidate_manifest.clone(),
        });
    }
    if baseline.mode != candidate.mode {
        return Err(CompareError::Mode);
    }
    if baseline.run_id.is_none() || baseline.run_id != candidate.run_id {
        return Err(CompareError::RunId);
    }
    if baseline.planned_repetitions != candidate.planned_repetitions
        || baseline.planned_warm_ups != candidate.planned_warm_ups
    {
        return Err(CompareError::Attempts {
            id: baseline
                .questions
                .first()
                .map_or_else(String::new, |row| row.id.clone()),
        });
    }
    let (Some(baseline_route), Some(candidate_route)) = (
        baseline
            .questions
            .first()
            .and_then(|row| row.route.as_deref()),
        candidate
            .questions
            .first()
            .and_then(|row| row.route.as_deref()),
    ) else {
        return Err(CompareError::Route);
    };
    if baseline_route != candidate_route
        || baseline
            .questions
            .iter()
            .any(|row| row.route.as_deref() != Some(baseline_route))
        || candidate
            .questions
            .iter()
            .any(|row| row.route.as_deref() != Some(candidate_route))
    {
        return Err(CompareError::Route);
    }
    Ok(())
}

/// Pair one row per question, preserving the original v1 behavior.
fn pair_v1<'a>(
    baseline: &'a [QuestionResult],
    candidate: &'a [QuestionResult],
) -> Result<Vec<PairedCluster<'a>>, CompareError> {
    let baselines = by_id(baseline)?;
    let candidates = by_id(candidate)?;
    if let Some(&id) = candidates.keys().find(|id| !baselines.contains_key(*id)) {
        return Err(CompareError::Unpaired { id: id.to_owned() });
    }
    baseline
        .iter()
        .map(|question| {
            let Some(&paired) = candidates.get(question.id.as_str()) else {
                return Err(CompareError::Unpaired {
                    id: question.id.clone(),
                });
            };
            if paired.answerable != question.answerable {
                return Err(CompareError::Answerability {
                    id: question.id.clone(),
                });
            }
            Ok((vec![question], vec![paired]))
        })
        .collect()
}

/// Pair v2 rows by question, validating scheduled seeds while pooling retries.
fn pair_v2<'a>(
    baseline: &'a [QuestionResult],
    candidate: &'a [QuestionResult],
) -> Result<Vec<PairedCluster<'a>>, CompareError> {
    let baselines = by_question(baseline);
    let candidates = by_question(candidate);
    if let Some(&id) = candidates.keys().find(|id| !baselines.contains_key(*id)) {
        return Err(CompareError::Unpaired { id: id.to_owned() });
    }
    let mut clusters = Vec::with_capacity(baselines.len());
    for (&id, before) in &baselines {
        let Some(after) = candidates.get(id) else {
            return Err(CompareError::Unpaired { id: id.to_owned() });
        };
        unique_attempts(before, id)?;
        unique_attempts(after, id)?;
        let before_first = before.first().ok_or(CompareError::Version)?;
        let after_first = after.first().ok_or(CompareError::Version)?;
        if before
            .iter()
            .any(|row| row.answerable != before_first.answerable)
            || after
                .iter()
                .any(|row| row.answerable != after_first.answerable)
            || before_first.answerable != after_first.answerable
        {
            return Err(CompareError::Answerability { id: id.to_owned() });
        }
        if before.iter().any(|row| {
            row.language != before_first.language || row.cross_lingual != before_first.cross_lingual
        }) || after.iter().any(|row| {
            row.language != after_first.language || row.cross_lingual != after_first.cross_lingual
        }) || before_first.language != after_first.language
            || before_first.cross_lingual != after_first.cross_lingual
        {
            return Err(CompareError::Labels { id: id.to_owned() });
        }
        if scheduled_attempts(before, id)? != scheduled_attempts(after, id)? {
            return Err(CompareError::Attempts { id: id.to_owned() });
        }
        let baseline_scored = before.iter().any(|row| row.warm_up != Some(true));
        let candidate_scored = after.iter().any(|row| row.warm_up != Some(true));
        if baseline_scored && candidate_scored {
            clusters.push((before.clone(), after.clone()));
        }
    }
    Ok(clusters)
}

/// Refuses duplicate attempt identities within one question cluster.
fn unique_attempts(rows: &[&QuestionResult], id: &str) -> Result<(), CompareError> {
    let mut attempts = BTreeSet::new();
    for row in rows {
        let Some(attempt) = row.attempt else {
            return Err(CompareError::Version);
        };
        if !attempts.insert(attempt) {
            return Err(CompareError::Attempts { id: id.to_owned() });
        }
    }
    Ok(())
}

/// Scheduled rows by repetition, seed and warm-up flag; retries may differ.
fn scheduled_attempts(
    rows: &[&QuestionResult],
    id: &str,
) -> Result<BTreeMap<(u32, u64, bool), usize>, CompareError> {
    let mut scheduled = BTreeMap::new();
    for row in rows.iter().filter(|row| row.retry_of.is_none()) {
        let (Some(repetition), Some(seed), Some(warm_up)) = (row.repetition, row.seed, row.warm_up)
        else {
            return Err(CompareError::Version);
        };
        *scheduled.entry((repetition, seed, warm_up)).or_default() += 1;
    }
    if scheduled.is_empty() {
        return Err(CompareError::Attempts { id: id.to_owned() });
    }
    Ok(scheduled)
}

/// Rows of a v2 run, grouped by question ID.
fn by_question(questions: &[QuestionResult]) -> BTreeMap<&str, Vec<&QuestionResult>> {
    let mut grouped = BTreeMap::new();
    for question in questions {
        grouped
            .entry(question.id.as_str())
            .or_insert_with(Vec::new)
            .push(question);
    }
    grouped
}

/// The questions of a v1 run by ID.
///
/// # Errors
///
/// [`CompareError::Repeated`] for an ID given twice.
fn by_id(questions: &[QuestionResult]) -> Result<BTreeMap<&str, &QuestionResult>, CompareError> {
    let mut by_id = BTreeMap::new();
    for question in questions {
        if by_id.insert(question.id.as_str(), question).is_some() {
            return Err(CompareError::Repeated {
                id: question.id.clone(),
            });
        }
    }
    Ok(by_id)
}
