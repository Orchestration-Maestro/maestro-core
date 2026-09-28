//! The ladder's run: every rung's cards checked first, then each rung in
//! turn, its warm-ups unscored, each question searched then asked, each whole
//! operation timed, and the rung scored. A rung whose generation or cards
//! differ between its start and its end, or from the first rung's start, the
//! reranker, the answerer and the prompt aside, is INVALID.

use super::manifest::Rung;
use crate::failure::Failure;
use maestro_knowledge::{
    eval::{
        Ask, AskOutcome, DeliveryScore, LadderQuestion, LadderScore, Search, SearchOutcome,
        SectionRef, score_delivery, score_ladder,
    },
    search::evidence::Anchor,
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
    /// The digest of the rung's answerer card, absent when none is
    /// registered.
    pub(super) answerer: Option<String>,
    /// The SHA-256 of the rung's prompt file, absent when it asks with a
    /// prompt version or does not ask.
    pub(super) prompt: Option<String>,
}

impl Provenance {
    /// Whether it runs against the same generation, chunk set and embedder
    /// as `other`: rungs differ in their rerankers, answerers and prompts
    /// only.
    pub(super) fn matches(&self, other: &Self) -> bool {
        self.generation == other.generation
            && self.chunk_set == other.chunk_set
            && self.embedder == other.embedder
    }
}

/// What a search gave the ladder.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Searched {
    /// How it ended: when ranked, the distinct documents of its final ranked
    /// chunks, before evidence assembly, the first 10, which the floors score.
    pub(super) outcome: SearchOutcome,
    /// The anchors of its evidence, assembled under the rung's ask budget:
    /// scored when the rung does not ask.
    pub(super) delivered: Vec<Anchor>,
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
    /// The top fused score, absent when no fused candidate was loaded.
    pub(super) top_fused_score: Option<f64>,
    /// Source-context loading and validation wall time in microseconds.
    pub(super) candidate_source_load_micros: u64,
    /// Candidate identities whose oversized source units retained indexed input.
    pub(super) candidate_context_fallbacks: Vec<String>,
}

/// What an `ask` gave the ladder.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Asked {
    /// How it ended.
    pub(super) outcome: AskOutcome,
    /// The anchors of the bundle its answerer was given, none when it
    /// failed before one was assembled.
    pub(super) delivered: Vec<Anchor>,
    /// The attempts the answer check refused before it ended, never scored.
    pub(super) rejections: Vec<RejectedCheck>,
}

/// An attempt the answer check refused: the attempt and the check's code,
/// never the reply's tokens, which are answer text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct RejectedCheck {
    /// The attempt, from 1.
    pub(super) attempt: u8,
    /// The stable code of the failed check, such as `unsupported_literal`.
    pub(super) check: &'static str,
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
    fn ask(&self, rung: &Rung, question: &str) -> Asked;
}

/// One rung's run.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RungRun {
    /// The rung.
    pub(super) rung: Rung,
    /// What it ran against at its start.
    pub(super) start: Provenance,
    /// What it ran against at its end, absent when that could not be read.
    pub(super) end: Option<Provenance>,
    /// What the ladder's first rung ran against at its start.
    pub(super) ladder: Provenance,
    /// The questions it ran first, unscored.
    pub(super) warm_ups: usize,
    /// Each scored question's row, in the suite's order.
    pub(super) rows: Vec<LadderQuestion>,
    /// Each row's search diagnostic.
    pub(super) diagnostics: Vec<SearchDiagnostic>,
    /// Each row's attempts the answer check refused, none when the rung
    /// does not ask.
    pub(super) rejections: Vec<Vec<RejectedCheck>>,
    /// The floors.
    pub(super) score: LadderScore,
    /// What the evidence the answerer received delivered, beside the floors.
    pub(super) delivery: DeliveryScore,
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
    /// first rung's, or what it ran against at its end could not be read.
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
        if self.end.as_ref() != Some(&self.start) || !self.start.matches(&self.ladder) {
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
/// `record`, which stop the ladder; and the error of reading what a rung ran
/// against at its end, which stops the ladder once that rung, INVALID, is
/// recorded.
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
        let (run, end_failure) = run_rung(engine, suite, warm_ups, rung, ladder)?;
        record(&run)?;
        if let Some(failure) = end_failure {
            return Err(failure);
        }
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
    if rung.ask.is_some() && provenance.answerer.is_none() {
        return Err(Failure::refused(format!(
            "the rung `{}` asks, but the collection has no registered answerer card",
            rung.name
        )));
    }
    Ok(())
}

/// Runs `rung`: its warm-ups, then every question, then its score; with the
/// failure to read what it ran against at its end, if any.
fn run_rung(
    engine: &mut impl Engine,
    suite: &Suite,
    warm_ups: usize,
    rung: &Rung,
    ladder: Option<Provenance>,
) -> Result<(RungRun, Option<Failure>), Failure> {
    let (start, expected) = engine.start(rung, suite)?;
    if expected.len() != suite.questions.len() {
        return Err(Failure::refused(format!(
            "the rung `{}` resolved expected sections for {} of the suite's {} questions",
            rung.name,
            expected.len(),
            suite.questions.len()
        )));
    }
    for question in suite.questions.iter().take(warm_ups) {
        engine.search(rung, &question.question);
        if rung.ask.is_some() {
            engine.ask(rung, &question.question);
        }
    }
    let mut rejections = Vec::with_capacity(suite.questions.len());
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
            let (asked, elapsed) = if rung.ask.is_some() {
                timed(|| engine.ask(rung, &question.question))
            } else {
                let unasked = Asked {
                    outcome: AskOutcome::Failed,
                    delivered: searched.delivered,
                    rejections: Vec::new(),
                };
                (unasked, Duration::ZERO)
            };
            rejections.push(asked.rejections);
            let outcome = asked.outcome;
            let row = LadderQuestion {
                id: question.id.clone(),
                expected,
                search,
                ask: Ask { outcome, elapsed },
                delivered: asked.delivered,
            };
            (row, searched.diagnostic)
        })
        .unzip();
    let (end, end_failure) = match engine.provenance(rung) {
        Ok(end) => (Some(end), None),
        Err(failure) => (None, Some(failure)),
    };
    let score = score_ladder(suite, &rows);
    let delivery = score_delivery(suite, &rows, rung.ask.is_some());
    let score = if rung.ask.is_some() {
        score
    } else {
        score.without_asks()
    };
    let run = RungRun {
        rung: rung.clone(),
        ladder: ladder.unwrap_or_else(|| start.clone()),
        start,
        end,
        warm_ups,
        rows,
        diagnostics,
        rejections,
        score,
        delivery,
    };
    Ok((run, end_failure))
}

/// What `operation` gives, and how long it took.
fn timed<T>(operation: impl FnOnce() -> T) -> (T, Duration) {
    let started = Instant::now();
    let value = operation();
    (value, started.elapsed())
}
