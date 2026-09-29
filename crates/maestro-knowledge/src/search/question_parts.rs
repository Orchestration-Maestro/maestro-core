//! Compound questions ranked part by part, behind a replaceable splitter
//! port. A question such as "send a message only after the third failure"
//! asks for an action and a condition; each part is ranked against its own
//! words beside the whole question, and the bundle keeps each part's best
//! passage.

use crate::query::Understood;
use maestro_kernel::evidence::RouteStatus;
use serde::{Deserialize, Serialize};

/// Whether search also ranks a compound question's parts.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuestionParts {
    /// The whole question only, as before.
    #[default]
    Off,
    /// The whole question, and each part a splitter finds.
    Split,
}

/// Why a relation phrase did not start a part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsplit {
    /// The question holds no relation phrase.
    NoRelation,
    /// A part would hold too few words of its own.
    ShortPart,
    /// The phrase sits inside a quote, an identifier or a name.
    Protected,
    /// The question already has the most parts search ranks.
    PartCap,
}

impl Unsplit {
    /// The stable code explain and trace record.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NoRelation => "no_relation_phrase",
            Self::ShortPart => "part_too_short",
            Self::Protected => "protected_phrase",
            Self::PartCap => "part_cap",
        }
    }
}

/// A question's parts, in reading order, and why it has no more.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestionSplit {
    /// The parts; one, the question itself, when it is not split.
    pub parts: Vec<String>,
    /// Why the first relation phrase that starts no part did not, if any.
    pub unsplit: Option<Unsplit>,
}

/// Splits a question into the parts search ranks one by one.
pub trait QuestionSplitter: Send + Sync {
    /// The parts of `question`.
    fn split(&self, question: &Understood) -> QuestionSplit;
}

/// What search did with one part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartRecord {
    /// The part's words, the question's topic included.
    pub text: String,
    /// The documentation words its bridge added, empty when none.
    pub bridge: String,
    /// How its bridge ended; absent when search has no bridge.
    pub bridge_status: Option<RouteStatus>,
    /// Whether its ranking ran, or why it fell back to the whole question.
    pub status: RouteStatus,
    /// Its best passage's chunk, which the bundle reserves; absent when it
    /// ranked nothing.
    pub best: Option<String>,
}

/// What search did with a question under [`QuestionParts::Split`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartsRecord {
    /// The whole question's best passage's chunk, which the bundle reserves
    /// before each part's; absent when the question was not split or its
    /// ranking is empty.
    pub whole: Option<String>,
    /// Each part, in reading order; empty when the question was not split.
    pub parts: Vec<PartRecord>,
    /// Why the question has no more parts, if a reason applies.
    pub unsplit: Option<Unsplit>,
}

impl PartsRecord {
    /// The chunks the bundle reserves, in order: the whole question's best
    /// passage's, then each part's.
    pub fn reserved(&self) -> impl Iterator<Item = &str> {
        self.whole
            .as_deref()
            .into_iter()
            .chain(self.parts.iter().filter_map(|part| part.best.as_deref()))
    }
}
