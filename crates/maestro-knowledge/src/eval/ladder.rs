//! The ladder's scorer (T037): the M1 floors of one configuration, each PASS
//! or FAIL, from what search and `ask` gave for every question of the suite.
//!
//! The floors are scored in code, with no human grade. The suite says which
//! questions exist and which are answerable, so it fixes every denominator.
//! Each row names its question by ID and gives the sections it expects,
//! resolved in the evaluated generation, and the outcome and time of its
//! search and its `ask` ([`LadderQuestion`]). A question without a row failed
//! both. A row the suite does not know, or a second row of one question, makes
//! every floor unavailable: dropping a question or keeping the best of two
//! attempts never moves a floor.
//!
//! - right document top-10 and first: an expected document within the first
//!   10, or first, of the search's ranked documents, over the answerable
//!   questions: at least 90%, and 70%;
//! - unanswerable refused: the unanswerable questions `ask` refused for a
//!   reason about the question (`not_found`, `no_evidence`, `unsupported`),
//!   over them: at least 90%. An unavailable answerer, a failure or a timeout
//!   is no refusal;
//! - right-section citation: the delivered answers citing a section an
//!   answerable question expects, over every delivered answer, answers to
//!   unanswerable questions included: at least 90%. A section matches when
//!   its document and pinned revision match and the citation names the same
//!   section ID or its span intersects the section's extent (half-open:
//!   spans that only touch do not); a document expected whole matches any
//!   citation of it in that revision;
//! - answerable answered right: the answerable questions answered with such a
//!   citation, over them: at least [`ANSWERED_PERCENT`]. A refusal, a failure
//!   or a timeout counts against it;
//! - invented literals: none in any delivered answer, as the answer check
//!   counts them;
//! - p95 search at most 1.5 s and p95 `ask` at most 10 s: the nearest-rank
//!   95th percentile over every question, failures included; a timeout or a
//!   missing row ranks above every time, so it counts against the limit.
//!
//! "At least p%" of n means at least ceil(p n / 100), in integers. A share of
//! no questions is unavailable, and so is the citation floor when no answer
//! was delivered; the score passes only when every floor passes. Apart from
//! the floors, the score counts the supported answers, answerable questions
//! answered with a right citation and no invented literal, the false
//! refusals, and the failed searches and asks.

use super::{bootstrap::percentile, error::RunError, run::resolve_details};
use crate::{answer::RefusalCode, search::evidence::Anchor, suite::Suite};
use maestro_canonicalization::CanonicalDocument;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    time::Duration,
};

/// A section, or a document whole, in the evaluated generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionRef {
    /// The ID of the document.
    pub document_id: String,
    /// The pinned revision of the document.
    pub revision_id: Option<String>,
    /// The ID of the cited chunk; always absent on an expected section.
    pub chunk_id: Option<String>,
    /// The ID of the section, absent for the document whole.
    pub section_id: Option<String>,
    /// The half-open byte range in the document's source: on a citation, the
    /// cited passage's span; on an expected section, the section's extent.
    pub span: Option<[usize; 2]>,
    /// The part of a composed answer an expected section gives, as the suite
    /// names it; always absent on a citation.
    pub component: Option<String>,
}

impl SectionRef {
    /// The section `section_id` of the document `document_id`.
    #[must_use]
    pub fn section(document_id: &str, section_id: &str) -> Self {
        Self {
            document_id: document_id.to_owned(),
            revision_id: None,
            chunk_id: None,
            section_id: Some(section_id.to_owned()),
            span: None,
            component: None,
        }
    }

    /// The document `document_id`, whole.
    #[must_use]
    pub fn document(document_id: &str) -> Self {
        Self {
            document_id: document_id.to_owned(),
            revision_id: None,
            chunk_id: None,
            section_id: None,
            span: None,
            component: None,
        }
    }

    /// Whether `citation` cites this expected section, or this document.
    fn cited_by(&self, citation: &Self) -> bool {
        let same_revision = self.revision_id == citation.revision_id;
        self.document_id == citation.document_id
            && same_revision
            && (self.section_id.is_none()
                || self.section_id == citation.section_id
                || self
                    .span
                    .zip(citation.span)
                    .is_some_and(|(expected, cited)| {
                        expected[0].max(cited[0]) < expected[1].min(cited[1])
                    }))
    }
}

/// What search and `ask` gave for one question of the suite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LadderQuestion {
    /// The question's ID in the suite.
    pub id: String,
    /// The sections that answer it, resolved; any one of them is right.
    pub expected: Vec<SectionRef>,
    /// Its search.
    pub search: Search,
    /// Its `ask`.
    pub ask: Ask,
    /// The anchors of the evidence the answerer received: the bundle its
    /// `ask` answered from, or, when the configuration does not ask, the one
    /// its search assembled under the ask budget. Scored apart from the
    /// floors.
    pub delivered: Vec<Anchor>,
}

/// A search and the time it took, from request to result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Search {
    /// How it ended.
    pub outcome: SearchOutcome,
    /// How long it took.
    pub elapsed: Duration,
}

/// How a search ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchOutcome {
    /// The IDs of the documents of its final ranked slots, in rank order.
    Ranked(Vec<String>),
    /// It failed, or a stage its configuration enables did not run, even
    /// when a fallback still ranked documents.
    Failed,
    /// It ran out of time, or a stage its configuration enables did.
    TimedOut,
}

/// An `ask` and the time it took, from request to answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    /// How it ended.
    pub outcome: AskOutcome,
    /// How long it took.
    pub elapsed: Duration,
}

/// How an `ask` ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskOutcome {
    /// It delivered an answer.
    Answered {
        /// The sections the answer cites.
        citations: Vec<SectionRef>,
        /// The literals of the answer its sources do not hold, as the answer
        /// check counts them.
        invented_literals: u32,
    },
    /// It refused to answer, with this code.
    Refused(RefusalCode),
    /// It failed, or a stage its configuration enables did not run in its
    /// search, even when a fallback still answered.
    Failed,
    /// It ran out of time, or a stage its configuration enables did.
    TimedOut,
}

/// One M1 floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Floor {
    /// An expected document in the first 10 ranked slots.
    #[serde(rename = "top_10")]
    Top10,
    /// An expected document in the first ranked slot.
    #[serde(rename = "top_1")]
    Top1,
    /// Unanswerable questions refused.
    Refused,
    /// Delivered answers citing an expected section.
    Citation,
    /// Answerable questions answered citing an expected section.
    Answered,
    /// Invented literals in delivered answers.
    Literals,
    /// The p95 of search.
    SearchP95,
    /// The p95 of `ask`.
    AskP95,
}

/// Whether a floor holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FloorStatus {
    /// It holds.
    Pass,
    /// It does not.
    Fail,
    /// Nothing measures it, or the rows do not match the suite.
    Unavailable,
}

/// What a floor measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Measure {
    /// A share of questions or answers.
    Share {
        /// How many meet it.
        count: usize,
        /// Of how many.
        of: usize,
        /// The least share that passes, in percent.
        percent: usize,
        /// The least count that passes.
        required: usize,
    },
    /// The invented literals.
    Literals {
        /// How many the delivered answers hold.
        invented: u64,
        /// The delivered answers.
        answers: usize,
    },
    /// A nearest-rank p95.
    Latency {
        /// The p95 in microseconds, absent when it did not end.
        p95_us: Option<u64>,
        /// Whether the p95 is a timeout or a question without a row.
        p95_unended: bool,
        /// The operations that timed out.
        timeouts: usize,
        /// The questions.
        of: usize,
        /// The most the p95 may be, in microseconds.
        limit_us: u64,
    },
    /// Nothing: the operation it measures did not run.
    NotRun {
        /// Always false.
        ran: bool,
    },
}

/// A floor, what it measured and whether it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FloorResult {
    /// The floor.
    pub floor: Floor,
    /// Whether it holds.
    pub status: FloorStatus,
    /// What it measured.
    #[serde(flatten)]
    pub measure: Measure,
}

/// The floors of one configuration, `maestro-eval-ladder/1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LadderScore {
    /// The contract the score follows.
    pub schema: &'static str,
    /// Whether every floor passes.
    pub passed: bool,
    /// Every floor, in the order of [`Floor`].
    pub floors: Vec<FloorResult>,
    /// The answerable questions answered with a right citation and no
    /// invented literal.
    pub supported_answers: usize,
    /// The answerable questions.
    pub answerable: usize,
    /// The answerable questions `ask` refused for a reason about the question.
    pub false_refusals: usize,
    /// The questions whose search failed, timed out or has no row.
    pub failed_searches: usize,
    /// Whether `ask` ran; when it did not, its floors are not run.
    pub asked: bool,
    /// The questions whose `ask` failed, timed out, found no answerer or has
    /// no row; 0 when `ask` did not run.
    pub failed_asks: usize,
    /// The questions without a row.
    pub missing: usize,
    /// The IDs of the rows the suite does not know, or that repeat an earlier
    /// row's question.
    pub rejected_ids: Vec<String>,
    /// The counts of each language of the suite, French first.
    pub languages: Vec<LanguageCounts>,
}

/// One language's counts; those `ask` gives are absent when it did not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LanguageCounts {
    /// The language, `fr` or `en`.
    pub language: &'static str,
    /// Its answerable questions.
    pub answerable: usize,
    /// Those with an expected document in the first 10 ranked slots.
    pub top_10: usize,
    /// Its unanswerable questions.
    pub unanswerable: usize,
    /// Those `ask` refused for a reason about the question.
    pub refused: Option<usize>,
    /// The answerable questions `ask` refused for a reason about the question.
    pub false_refusals: Option<usize>,
    /// The answerable questions answered with a right citation and no
    /// invented literal.
    pub supported_answers: Option<usize>,
}

/// The least share of answerable questions with an expected document in the
/// first 10 ranked slots, in percent.
const TOP_10_PERCENT: usize = 90;
/// The least share with one in the first slot, in percent.
const TOP_1_PERCENT: usize = 70;
/// The least share of unanswerable questions refused, in percent.
const REFUSED_PERCENT: usize = 90;
/// The least share of delivered answers citing a right section, in percent.
const CITATION_PERCENT: usize = 90;
/// The least share of answerable questions answered with a right citation,
/// in percent (owner ruling, 2026-09-28).
pub const ANSWERED_PERCENT: usize = 80;
/// The most the p95 of search may be.
const SEARCH_P95_LIMIT: Duration = Duration::from_millis(1500);
/// The most the p95 of `ask` may be.
const ASK_P95_LIMIT: Duration = Duration::from_secs(10);

/// The sections and documents each question of `suite` expects, in the
/// suite's order, resolved in the canonical documents `documents` gives by
/// `source_ref`, as [`run()`](super::run()) resolves them.
///
/// # Errors
///
/// The errors of [`run()`](super::run()) before any retrieval.
pub fn resolve_expected<E>(
    suite: &Suite,
    mut documents: impl FnMut(&str) -> Result<Option<CanonicalDocument>, E>,
) -> Result<Vec<Vec<SectionRef>>, RunError<E>> {
    Ok(resolve_details(suite, &mut documents)?
        .into_iter()
        .map(|expected| {
            expected
                .into_iter()
                .map(|item| SectionRef {
                    document_id: item.expected.document_id,
                    revision_id: Some(item.revision_id),
                    chunk_id: None,
                    section_id: item.expected.section_id,
                    span: item.span,
                    component: item.component,
                })
                .collect()
        })
        .collect())
}

/// One question of the suite and its row, if any.
#[derive(Debug, Clone, Copy)]
struct Scored<'row> {
    /// Whether the suite says a section answers it.
    answerable: bool,
    /// Its row.
    row: Option<&'row LadderQuestion>,
}

/// Scores every floor of the questions of `suite` from `rows`, with no
/// count per language.
pub(super) fn score_floors(suite: &Suite, rows: &[LadderQuestion]) -> LadderScore {
    let (by_id, rejected_ids) = index(suite, rows);
    let entries: Vec<Scored<'_>> = suite
        .questions
        .iter()
        .map(|question| Scored {
            answerable: question.answerable,
            row: by_id.get(question.id.as_str()).copied(),
        })
        .collect();
    let (answerable, unanswerable): (Vec<Scored<'_>>, Vec<Scored<'_>>) =
        entries.iter().partition(|entry| entry.answerable);
    let answers: Vec<(Scored<'_>, &[SectionRef], u32)> = entries
        .iter()
        .filter_map(|entry| {
            entry
                .answer()
                .map(|(cited, invented)| (*entry, cited, invented))
        })
        .collect();
    let right_answers = count(&answers, |(entry, cited, _)| entry.cites_right(cited));
    let mut floors = vec![
        share(
            Floor::Top10,
            count(&answerable, |entry| entry.ranks_within(10)),
            answerable.len(),
            TOP_10_PERCENT,
        ),
        share(
            Floor::Top1,
            count(&answerable, |entry| entry.ranks_within(1)),
            answerable.len(),
            TOP_1_PERCENT,
        ),
        share(
            Floor::Refused,
            count(&unanswerable, Scored::refused),
            unanswerable.len(),
            REFUSED_PERCENT,
        ),
        share(
            Floor::Citation,
            right_answers,
            answers.len(),
            CITATION_PERCENT,
        ),
        share(
            Floor::Answered,
            right_answers,
            answerable.len(),
            ANSWERED_PERCENT,
        ),
        literals(&answers),
        latency(
            Floor::SearchP95,
            entries.iter().map(Scored::search_sample),
            SEARCH_P95_LIMIT,
        ),
        latency(
            Floor::AskP95,
            entries.iter().map(Scored::ask_sample),
            ASK_P95_LIMIT,
        ),
    ];
    if !rejected_ids.is_empty() {
        for result in &mut floors {
            result.status = FloorStatus::Unavailable;
        }
    }
    LadderScore {
        schema: "maestro-eval-ladder/1",
        passed: floors
            .iter()
            .all(|result| result.status == FloorStatus::Pass),
        floors,
        supported_answers: count(&answers, |(entry, cited, invented)| {
            *invented == 0 && entry.cites_right(cited)
        }),
        answerable: answerable.len(),
        false_refusals: count(&answerable, Scored::refused),
        failed_searches: count(&entries, Scored::search_failed),
        asked: true,
        failed_asks: count(&entries, Scored::ask_failed),
        missing: count(&entries, |entry| entry.row.is_none()),
        rejected_ids,
        languages: Vec::new(),
    }
}

/// The rows of `rows` by the ID of their question in `suite`, and the IDs of
/// the rows it does not know or that repeat an earlier row's question.
pub(super) fn index<'row>(
    suite: &Suite,
    rows: &'row [LadderQuestion],
) -> (BTreeMap<&'row str, &'row LadderQuestion>, Vec<String>) {
    let known: BTreeSet<&str> = suite
        .questions
        .iter()
        .map(|question| question.id.as_str())
        .collect();
    let mut by_id = BTreeMap::new();
    let mut rejected = Vec::new();
    for row in rows {
        match by_id.entry(row.id.as_str()) {
            Entry::Vacant(vacant) if known.contains(row.id.as_str()) => {
                vacant.insert(row);
            }
            Entry::Vacant(_) | Entry::Occupied(_) => rejected.push(row.id.clone()),
        }
    }
    (by_id, rejected)
}

impl<'row> Scored<'row> {
    /// Whether its search ranks an expected document within its first
    /// `slots`.
    fn ranks_within(&self, slots: usize) -> bool {
        let Some(row) = self.row else {
            return false;
        };
        let SearchOutcome::Ranked(documents) = &row.search.outcome else {
            return false;
        };
        documents.iter().take(slots).any(|document| {
            row.expected
                .iter()
                .any(|expected| &expected.document_id == document)
        })
    }

    /// The citations and invented literals of its answer, if `ask` delivered
    /// one.
    fn answer(&self) -> Option<(&'row [SectionRef], u32)> {
        match &self.row?.ask.outcome {
            AskOutcome::Answered {
                citations,
                invented_literals,
            } => Some((citations, *invented_literals)),
            AskOutcome::Refused(_) | AskOutcome::Failed | AskOutcome::TimedOut => None,
        }
    }

    /// Whether it is answerable and one of `citations` cites a section it
    /// expects.
    fn cites_right(&self, citations: &[SectionRef]) -> bool {
        self.answerable
            && self.row.is_some_and(|row| {
                citations.iter().any(|citation| {
                    row.expected
                        .iter()
                        .any(|expected| expected.cited_by(citation))
                })
            })
    }

    /// Whether `ask` refused it for a reason about the question.
    fn refused(&self) -> bool {
        self.row.is_some_and(|row| match row.ask.outcome {
            AskOutcome::Refused(code) => about_the_question(code),
            AskOutcome::Answered { .. } | AskOutcome::Failed | AskOutcome::TimedOut => false,
        })
    }

    /// Whether its search failed, timed out or has no row.
    fn search_failed(&self) -> bool {
        self.row
            .is_none_or(|row| !matches!(row.search.outcome, SearchOutcome::Ranked(_)))
    }

    /// Whether its `ask` failed, timed out, found no answerer or has no row.
    fn ask_failed(&self) -> bool {
        self.row.is_none_or(|row| match row.ask.outcome {
            AskOutcome::Refused(code) => !about_the_question(code),
            AskOutcome::Failed | AskOutcome::TimedOut => true,
            AskOutcome::Answered { .. } => false,
        })
    }

    /// Its search's place in the latency ranking.
    fn search_sample(&self) -> Sample {
        self.row
            .map_or(Sample::Missing, |row| match row.search.outcome {
                SearchOutcome::TimedOut => Sample::TimedOut,
                SearchOutcome::Ranked(_) | SearchOutcome::Failed => {
                    Sample::Ended(row.search.elapsed)
                }
            })
    }

    /// Its `ask`'s place in the latency ranking.
    fn ask_sample(&self) -> Sample {
        self.row
            .map_or(Sample::Missing, |row| match row.ask.outcome {
                AskOutcome::TimedOut => Sample::TimedOut,
                AskOutcome::Answered { .. } | AskOutcome::Refused(_) | AskOutcome::Failed => {
                    Sample::Ended(row.ask.elapsed)
                }
            })
    }
}

/// Whether a refusal with `code` is about the question, and so a refusal the
/// floors count; one about the answerer is a failure.
const fn about_the_question(code: RefusalCode) -> bool {
    match code {
        RefusalCode::NotFound | RefusalCode::NoEvidence | RefusalCode::Unsupported => true,
        RefusalCode::AnswererUnavailable => false,
    }
}

/// How many of `items` meet `test`.
fn count<T>(items: &[T], test: impl Fn(&T) -> bool) -> usize {
    items.iter().filter(|item| test(item)).count()
}

/// The share floor `floor`: `count` of `of`, which needs at least `percent`.
fn share(floor: Floor, count: usize, of: usize, percent: usize) -> FloorResult {
    let required = (percent * of).div_ceil(100);
    let status = if of == 0 {
        FloorStatus::Unavailable
    } else if count >= required {
        FloorStatus::Pass
    } else {
        FloorStatus::Fail
    };
    FloorResult {
        floor,
        status,
        measure: Measure::Share {
            count,
            of,
            percent,
            required,
        },
    }
}

/// The invented-literals floor over the delivered `answers`: none may hold
/// one.
fn literals(answers: &[(Scored<'_>, &[SectionRef], u32)]) -> FloorResult {
    let invented = answers
        .iter()
        .map(|(_, _, invented)| u64::from(*invented))
        .sum();
    FloorResult {
        floor: Floor::Literals,
        status: if invented == 0 {
            FloorStatus::Pass
        } else {
            FloorStatus::Fail
        },
        measure: Measure::Literals {
            invented,
            answers: answers.len(),
        },
    }
}

/// One operation's place in the latency ranking: every time below every
/// timeout, and every timeout below every question without a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Sample {
    /// It ended, in this time.
    Ended(Duration),
    /// It ran out of time.
    TimedOut,
    /// The question has no row.
    Missing,
}

/// The latency floor `floor` over `samples`, one per question: the
/// nearest-rank p95 may be at most `limit`.
fn latency(floor: Floor, samples: impl Iterator<Item = Sample>, limit: Duration) -> FloorResult {
    let mut samples: Vec<Sample> = samples.collect();
    samples.sort_unstable();
    let p95 = percentile(&samples, 950).unwrap_or(Sample::Missing);
    let p95_us = match p95 {
        Sample::Ended(elapsed) => Some(micros(elapsed)),
        Sample::TimedOut | Sample::Missing => None,
    };
    FloorResult {
        floor,
        status: match p95 {
            Sample::Ended(elapsed) if elapsed <= limit => FloorStatus::Pass,
            Sample::Ended(_) | Sample::TimedOut | Sample::Missing => FloorStatus::Fail,
        },
        measure: Measure::Latency {
            p95_us,
            p95_unended: p95_us.is_none(),
            timeouts: count(&samples, |sample| *sample == Sample::TimedOut),
            of: samples.len(),
            limit_us: micros(limit),
        },
    }
}

/// `duration` in whole microseconds, saturating.
fn micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}
