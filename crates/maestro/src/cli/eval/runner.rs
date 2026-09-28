//! The ladder's run: every rung's cards checked first, then each rung in
//! turn, its warm-ups unscored, each question searched then asked, each whole
//! operation timed, and the rung scored. A rung whose generation or cards
//! differ between its start and its end, or from the first rung's start, the
//! reranker aside, is INVALID.

use super::manifest::Rung;
use crate::failure::Failure;
use maestro_knowledge::{
    eval::{
        Ask, AskOutcome, LadderQuestion, LadderScore, Search, SearchOutcome, SectionRef,
        score_ladder,
    },
    suite::Suite,
};
use serde::Serialize;
use std::time::{Duration, Instant};

/// What a rung runs against: the collection's published generation and the
/// cards of its models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Provenance {
    /// The published generation's ID.
    pub(super) generation: i64,
    /// Its chunk set.
    pub(super) chunk_set: String,
    /// The digest of the embedder's card, absent when none matches.
    pub(super) embedder: Option<String>,
    /// The digest of the rung's reranker card, absent when reranking is off.
    pub(super) reranker: Option<String>,
    /// The digest of the answerer's card, absent when none is registered.
    pub(super) answerer: Option<String>,
}

impl Provenance {
    /// Whether it runs against the same generation, chunk set, embedder and
    /// answerer as `other`: rungs differ in their rerankers only.
    pub(super) fn matches(&self, other: &Self) -> bool {
        self.generation == other.generation
            && self.chunk_set == other.chunk_set
            && self.embedder == other.embedder
            && self.answerer == other.answerer
    }
}

/// What a search gave the ladder.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Searched {
    /// How it ended: when ranked, the distinct documents of its final ranked
    /// chunks, before evidence assembly, the first 10, which the floors score.
    pub(super) outcome: SearchOutcome,
    /// What else it gave, never scored.
    pub(super) diagnostic: SearchDiagnostic,
}

/// What a search gave beyond what the floors score: diagnostics only.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct SearchDiagnostic {
    /// The documents of its assembled evidence, in rank order.
    pub(super) bundle_documents: Vec<String>,
    /// The top reranker score, absent when rerank did not run.
    pub(super) top_rerank_score: Option<f64>,
    /// The top fused score, absent when nothing was fused.
    pub(super) top_fused_score: Option<f64>,
}

/// The machine a ladder runs on: the kernel, the router and the search
/// service, or fakes of them.
pub(super) trait Engine {
    /// What `rung` would run against now; it holds nothing.
    ///
    /// # Errors
    ///
    /// [`Failure::Refused`] for a rung whose cards cannot run, and
    /// [`Failure`] when the kernel cannot be read.
    fn provenance(&self, rung: &Rung) -> Result<Provenance, Failure>;

    /// Holds what `rung` runs against for its searches and asks, and gives
    /// it with the sections each question of `suite` expects there.
    ///
    /// # Errors
    ///
    /// As [`Engine::provenance`], and [`Failure::Refused`] for a suite whose
    /// expected sections do not resolve.
    fn start(
        &mut self,
        rung: &Rung,
        suite: &Suite,
    ) -> Result<(Provenance, Vec<Vec<SectionRef>>), Failure>;

    /// Searches `question` as `rung` configures.
    fn search(&self, rung: &Rung, question: &str) -> Searched;

    /// Asks `question` as `rung` configures.
    fn ask(&self, rung: &Rung, question: &str) -> AskOutcome;
}

/// One rung's run.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RungRun {
    /// The rung.
    pub(super) rung: Rung,
    /// What it ran against at its start.
    pub(super) start: Provenance,
    /// What it ran against at its end.
    pub(super) end: Provenance,
    /// What the ladder's first rung ran against at its start.
    pub(super) ladder: Provenance,
    /// The questions it ran first, unscored.
    pub(super) warm_ups: usize,
    /// Each scored question's row, in the suite's order.
    pub(super) rows: Vec<LadderQuestion>,
    /// Each row's search diagnostic.
    pub(super) diagnostics: Vec<SearchDiagnostic>,
    /// The floors.
    pub(super) score: LadderScore,
}

/// A rung's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(super) enum Verdict {
    /// Every floor passes.
    Pass,
    /// A floor does not pass.
    Fail,
    /// The generation or a card changed while it ran, or differs from the
    /// first rung's.
    Invalid,
}

impl Verdict {
    /// Its name in a report.
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Invalid => "INVALID",
        }
    }
}

impl RungRun {
    /// Its verdict.
    pub(super) fn verdict(&self) -> Verdict {
        if self.start != self.end || !self.start.matches(&self.ladder) {
            Verdict::Invalid
        } else if self.score.passed {
            Verdict::Pass
        } else {
            Verdict::Fail
        }
    }
}

/// Runs every rung of `rungs` over `suite` on `engine`, after checking that
/// each can run, with `warm_ups` unscored questions first, and hands each
/// rung's run to `record` as it ends.
///
/// # Errors
///
/// [`Failure::Refused`] for more warm-ups than questions and for a rung that
/// cannot run, before any search; the errors of [`Engine::start`] and of
/// `record`, which stop the ladder.
pub(super) fn run_ladder(
    engine: &mut impl Engine,
    suite: &Suite,
    warm_ups: usize,
    rungs: &[Rung],
    mut record: impl FnMut(&RungRun) -> Result<(), Failure>,
) -> Result<Vec<RungRun>, Failure> {
    if warm_ups > suite.questions.len() {
        return Err(Failure::refused(
            "the manifest asks for more warm-ups than the suite has questions",
        ));
    }
    for rung in rungs {
        check_cards(rung, &engine.provenance(rung)?)?;
    }
    let mut runs: Vec<RungRun> = Vec::with_capacity(rungs.len());
    for rung in rungs {
        let ladder = runs.first().map(|first| first.start.clone());
        let run = run_rung(engine, suite, warm_ups, rung, ladder)?;
        record(&run)?;
        runs.push(run);
    }
    Ok(runs)
}

/// Refuses `rung` when `provenance` lacks a card it needs: an embedder for
/// dense retrieval, an answerer for `ask`.
fn check_cards(rung: &Rung, provenance: &Provenance) -> Result<(), Failure> {
    if rung.configuration.routes.dense && provenance.embedder.is_none() {
        return Err(Failure::refused(format!(
            "the rung `{}` runs dense retrieval, but the generation has no embedder card",
            rung.name
        )));
    }
    if rung.ask && provenance.answerer.is_none() {
        return Err(Failure::refused(format!(
            "the rung `{}` asks, but the collection has no registered answerer card",
            rung.name
        )));
    }
    Ok(())
}

/// Runs `rung`: its warm-ups, then every question, then its score.
fn run_rung(
    engine: &mut impl Engine,
    suite: &Suite,
    warm_ups: usize,
    rung: &Rung,
    ladder: Option<Provenance>,
) -> Result<RungRun, Failure> {
    let (start, expected) = engine.start(rung, suite)?;
    for question in suite.questions.iter().take(warm_ups) {
        engine.search(rung, &question.question);
        if rung.ask {
            engine.ask(rung, &question.question);
        }
    }
    let (rows, diagnostics): (Vec<LadderQuestion>, Vec<SearchDiagnostic>) = suite
        .questions
        .iter()
        .zip(expected)
        .map(|(question, expected)| {
            let (searched, elapsed) = timed(|| engine.search(rung, &question.question));
            let search = Search {
                outcome: searched.outcome,
                elapsed,
            };
            let (outcome, elapsed) = if rung.ask {
                timed(|| engine.ask(rung, &question.question))
            } else {
                (AskOutcome::Failed, Duration::ZERO)
            };
            let row = LadderQuestion {
                id: question.id.clone(),
                expected,
                search,
                ask: Ask { outcome, elapsed },
            };
            (row, searched.diagnostic)
        })
        .unzip();
    let end = engine.provenance(rung)?;
    let score = score_ladder(suite, &rows);
    let score = if rung.ask {
        score
    } else {
        score.without_asks()
    };
    Ok(RungRun {
        rung: rung.clone(),
        ladder: ladder.unwrap_or_else(|| start.clone()),
        start,
        end,
        warm_ups,
        rows,
        diagnostics,
        score,
    })
}

/// What `operation` gives, and how long it took.
fn timed<T>(operation: impl FnOnce() -> T) -> (T, Duration) {
    let started = Instant::now();
    let value = operation();
    (value, started.elapsed())
}
