//! Comparing two runs of one suite, question by question.

use super::{
    bootstrap::estimates,
    error::CompareError,
    metric::{metrics, values},
    report::{Metrics, QuestionResult, Report},
};
use std::collections::BTreeMap;

/// Two runs compared: for each metric, the candidate's value minus the
/// baseline's, with its interval.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Comparison {
    /// The seed the paired resamples were drawn with.
    pub seed: u64,
    /// How many questions the two runs share.
    pub questions: usize,
    /// For each metric, the candidate's value minus the baseline's, over the
    /// questions the runs share, with the 95 % interval of that difference;
    /// absent when a run's metric covers no question.
    pub differences: Metrics,
}

/// The comparison of `candidate` with `baseline`, two runs over one
/// collection of the same suite, read from the same file, and questions:
/// each question of one is paired with the question of the other that has
/// its id, in the baseline's order. For each metric, it gives
/// the candidate's value minus the baseline's, with the 2.5th and 97.5th
/// percentiles of that difference over 2,000 resamples of the pairs, each
/// drawing as many answerable and unanswerable pairs as there are, with the
/// generator `seed` starts: the same seed gives the same comparison, bit for
/// bit.
///
/// # Errors
///
/// [`CompareError::Collection`] for runs over two collections,
/// [`CompareError::Suite`] for runs of two suites,
/// [`CompareError::SuiteDigest`] for runs of one suite read from files of
/// different digests, [`CompareError::Repeated`] for a question given twice
/// in one run, [`CompareError::Unpaired`] for a question in one run only,
/// and [`CompareError::Answerability`] for a question answerable in one run
/// only.
pub fn compare(
    baseline: &Report,
    candidate: &Report,
    seed: u64,
) -> Result<Comparison, CompareError> {
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
    let baselines = by_id(&baseline.questions)?;
    let candidates = by_id(&candidate.questions)?;
    if let Some(&id) = candidates.keys().find(|id| !baselines.contains_key(*id)) {
        return Err(CompareError::Unpaired { id: id.to_owned() });
    }
    let mut pairs = Vec::with_capacity(baseline.questions.len());
    for question in &baseline.questions {
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
        pairs.push((question, paired));
    }
    let answerable: Vec<bool> = pairs
        .iter()
        .map(|(question, _)| question.answerable)
        .collect();
    let found = estimates(&answerable, seed, |sample| {
        let (before, after): (Vec<&QuestionResult>, Vec<&QuestionResult>) = sample
            .iter()
            .filter_map(|&index| pairs.get(index).copied())
            .unzip();
        let before = values(&before);
        values(&after)
            .into_iter()
            .filter_map(|(metric, value)| Some((metric, value - before.get(&metric)?)))
            .collect()
    });
    Ok(Comparison {
        seed,
        questions: pairs.len(),
        differences: metrics(&found),
    })
}

/// The questions of a run by id.
///
/// # Errors
///
/// [`CompareError::Repeated`] for an id given twice.
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
