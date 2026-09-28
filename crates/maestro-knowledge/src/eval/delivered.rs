//! The ladder's delivery score: whether the sections a question accepts
//! reach the evidence its answerer receives, beside the floors, which score
//! ranked documents before assembly.
//!
//! Each row carries the anchors of that evidence ([`LadderQuestion::delivered`])
//! and is paired with its question by ID, as the floors pair them. A row is
//! credited only when its search ranked and, when the configuration asks, its
//! `ask` answered or refused: a search whose configured stage did not run, or
//! an `ask` that failed or timed out, delivers nothing.
//!
//! - delivered: the answerable questions with an accepted section delivered.
//!   An anchor delivers a section when its document and pinned revision match
//!   and it names the same section ID or its span intersects the section's
//!   extent (half-open: spans that only touch do not); a document expected
//!   whole is delivered by any anchor of its revision;
//! - fully delivered: those with an accepted section whose whole extent the
//!   union of the anchors' spans in its revision covers, since one shared byte,
//!   a heading line, already delivers a section; the coverage of a section is
//!   the share of its extent covered, that of a question its best section's,
//!   and the median is the nearest-rank one over the answerable questions. A
//!   section without an extent is covered whole when it is delivered;
//! - composition: the answerable questions naming two or more components, and
//!   those with every component delivered; the sections of one component are
//!   alternatives for it, and a section without a component is in none.

use super::{
    bootstrap::percentile,
    ladder::{AskOutcome, LadderQuestion, SearchOutcome, SectionRef, index},
};
use crate::{search::evidence::Anchor, suite::Suite};
use serde::Serialize;
use std::collections::BTreeMap;

/// What the evidence the answerer received delivered, over the answerable
/// questions of a suite.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct DeliveryScore {
    /// The answerable questions.
    pub answerable: usize,
    /// Those with an accepted section delivered.
    pub delivered: usize,
    /// Those with an accepted section delivered whole.
    pub fully_delivered: usize,
    /// The median of their best section's coverage, in thousandths, absent
    /// without an answerable question.
    pub median_coverage_permille: Option<u32>,
    /// Those that name two or more components.
    pub composition_cases: usize,
    /// Those of them with every component delivered.
    pub compositions_delivered: usize,
    /// The answerable questions without a row, or whose search did not rank
    /// or whose `ask` failed or timed out: credited with nothing.
    pub uncredited: usize,
}

/// Scores what each row of `rows` delivered for the questions of `suite`;
/// `asked` says whether the configuration asked, so that a failed `ask`
/// delivers nothing.
#[must_use]
pub fn score_delivery(suite: &Suite, rows: &[LadderQuestion], asked: bool) -> DeliveryScore {
    let (by_id, _) = index(suite, rows);
    let mut score = DeliveryScore::default();
    let mut coverages = Vec::new();
    for question in suite
        .questions
        .iter()
        .filter(|question| question.answerable)
    {
        score.answerable += 1;
        let Some(row) = by_id
            .get(question.id.as_str())
            .filter(|row| credited(row, asked))
        else {
            score.uncredited += 1;
            coverages.push(0);
            continue;
        };
        let anchors = row.delivered.as_slice();
        let delivered = row
            .expected
            .iter()
            .any(|section| delivers(section, anchors));
        score.delivered += usize::from(delivered);
        let coverage = row
            .expected
            .iter()
            .map(|section| coverage_permille(section, anchors))
            .max()
            .unwrap_or_default();
        score.fully_delivered += usize::from(coverage == FULL);
        coverages.push(coverage);
        if let Some(complete) = composition(&row.expected, anchors) {
            score.composition_cases += 1;
            score.compositions_delivered += usize::from(complete);
        }
    }
    coverages.sort_unstable();
    score.median_coverage_permille = percentile(&coverages, 500);
    score
}

/// A whole section's coverage, in thousandths.
const FULL: u32 = 1000;

/// Whether `row` is credited with what it delivered: its search ranked and,
/// when the configuration `asked`, its `ask` answered or refused.
fn credited(row: &LadderQuestion, asked: bool) -> bool {
    matches!(row.search.outcome, SearchOutcome::Ranked(_))
        && (!asked
            || matches!(
                row.ask.outcome,
                AskOutcome::Answered { .. } | AskOutcome::Refused(_)
            ))
}

/// Whether `anchor` lies in `section`'s document and pinned revision.
fn same_revision(section: &SectionRef, anchor: &Anchor) -> bool {
    section.document_id == anchor.doc_id
        && section.revision_id.as_deref() == Some(anchor.revision_id.as_str())
}

/// Whether one of `anchors` delivers `section`.
fn delivers(section: &SectionRef, anchors: &[Anchor]) -> bool {
    anchors.iter().any(|anchor| {
        same_revision(section, anchor)
            && (section.section_id.is_none()
                || section.section_id == anchor.section_id
                || section.span.is_some_and(|extent| {
                    extent[0].max(anchor.span[0]) < extent[1].min(anchor.span[1])
                }))
    })
}

/// The share of `section`'s extent that the union of `anchors`' spans in its
/// revision covers, in thousandths, rounded down; a section without an
/// extent, or with an empty one, is covered whole when it is delivered.
fn coverage_permille(section: &SectionRef, anchors: &[Anchor]) -> u32 {
    let Some([start, end]) = section.span.filter(|extent| extent[0] < extent[1]) else {
        return if delivers(section, anchors) { FULL } else { 0 };
    };
    let mut spans: Vec<[usize; 2]> = anchors
        .iter()
        .filter(|anchor| same_revision(section, anchor))
        .map(|anchor| [anchor.span[0].max(start), anchor.span[1].min(end)])
        .filter(|span| span[0] < span[1])
        .collect();
    spans.sort_unstable();
    let mut covered = 0;
    let mut reached = start;
    for [from, to] in spans {
        covered += to.saturating_sub(from.max(reached));
        reached = reached.max(to);
    }
    let permille = covered * 1000 / (end - start);
    u32::try_from(permille).unwrap_or(FULL)
}

/// Whether every component `expected` names is delivered, none when it names
/// fewer than two.
fn composition(expected: &[SectionRef], anchors: &[Anchor]) -> Option<bool> {
    let mut components: BTreeMap<&str, bool> = BTreeMap::new();
    for section in expected {
        if let Some(component) = section.component.as_deref() {
            *components.entry(component).or_default() |= delivers(section, anchors);
        }
    }
    (components.len() > 1).then(|| components.values().all(|delivered| *delivered))
}

impl DeliveryScore {
    /// The score as Markdown lines, each count over the answerable questions,
    /// with the rules it applies.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let median = self
            .median_coverage_permille
            .map_or_else(|| "none".to_owned(), tenths);
        format!(
            "- Delivered-section recall: {}/{} (same document and pinned revision; same \
             section ID or an anchor span intersecting the section's extent, half-open)\n\
             - Fully delivered sections: {}/{} (the union of the anchor spans covers an \
             accepted section's whole extent); median coverage {median}\n\
             - Required-composition coverage: {}/{}\n\
             - Uncredited questions: {} (no row, a search that did not rank, or an ask \
             that failed or timed out)\n",
            self.delivered,
            self.answerable,
            self.fully_delivered,
            self.answerable,
            self.compositions_delivered,
            self.composition_cases,
            self.uncredited
        )
    }
}

/// `permille` thousandths in percent, to one decimal.
fn tenths(permille: u32) -> String {
    format!("{}.{}%", permille / 10, permille % 10)
}
