//! The ladder's score as a configuration reports it: its floors with counts
//! per language of the suite, French and English apart, each language's
//! questions scored alone, counts only, since a floor holds over the whole
//! suite; and the score of a configuration that did not ask.

use super::ladder::{
    Floor, FloorStatus, LadderQuestion, LadderScore, LanguageCounts, Measure, score_floors,
};
use crate::suite::{Language, Suite};
use std::collections::BTreeSet;

/// Scores every floor of the questions of `suite` from `rows`, and counts
/// each language apart.
#[must_use]
pub fn score_ladder(suite: &Suite, rows: &[LadderQuestion]) -> LadderScore {
    LadderScore {
        languages: language_counts(suite, rows),
        ..score_floors(suite, rows)
    }
}

/// The counts of each language of `suite` from `rows`, French first.
fn language_counts(suite: &Suite, rows: &[LadderQuestion]) -> Vec<LanguageCounts> {
    [(Language::Fr, "fr"), (Language::En, "en")]
        .into_iter()
        .map(|(language, name)| {
            let alone = Suite {
                questions: suite
                    .questions
                    .iter()
                    .filter(|question| question.language == language)
                    .cloned()
                    .collect(),
                digest: suite.digest.clone(),
            };
            let ids: BTreeSet<&str> = alone
                .questions
                .iter()
                .map(|question| question.id.as_str())
                .collect();
            let rows: Vec<LadderQuestion> = rows
                .iter()
                .filter(|row| ids.contains(row.id.as_str()))
                .cloned()
                .collect();
            counts(name, &score_floors(&alone, &rows))
        })
        .collect()
}

/// The counts `name`'s questions scored alone give.
fn counts(name: &'static str, score: &LadderScore) -> LanguageCounts {
    let share = |floor: Floor| {
        score
            .floors
            .iter()
            .find(|result| result.floor == floor)
            .and_then(|result| match result.measure {
                Measure::Share { count, of, .. } => Some((count, of)),
                Measure::Literals { .. } | Measure::Latency { .. } | Measure::NotRun { .. } => None,
            })
            .unwrap_or_default()
    };
    let (refused, unanswerable) = share(Floor::Refused);
    LanguageCounts {
        language: name,
        answerable: score.answerable,
        top_10: share(Floor::Top10).0,
        unanswerable,
        refused: Some(refused),
        false_refusals: Some(score.false_refusals),
        supported_answers: Some(score.supported_answers),
    }
}

/// The floors `ask` measures.
const ASK_FLOORS: [Floor; 5] = [
    Floor::Refused,
    Floor::Citation,
    Floor::Answered,
    Floor::Literals,
    Floor::AskP95,
];

impl LadderScore {
    /// The score of a configuration that ran search alone: every floor
    /// `ask` measures is not run, so the score cannot pass, and no `ask`
    /// counts as failed.
    #[must_use]
    pub fn without_asks(mut self) -> Self {
        for result in &mut self.floors {
            if ASK_FLOORS.contains(&result.floor) {
                result.status = FloorStatus::Unavailable;
                result.measure = Measure::NotRun { ran: false };
            }
        }
        self.passed = false;
        self.asked = false;
        self.failed_asks = 0;
        for language in &mut self.languages {
            language.refused = None;
            language.false_refusals = None;
            language.supported_answers = None;
        }
        self
    }
}
