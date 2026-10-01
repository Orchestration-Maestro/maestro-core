//! Optional query expansion behind a replaceable port; generated text is
//! only ever a query, never evidence.

use crate::query::Understood;
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin};

/// Whether extra hypothetical-document routes are admitted.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentExpansion {
    /// Original deterministic retrieval only.
    #[default]
    Off,
    /// Add a guarded hypothetical passage and terminology query.
    Hyde,
}

/// When the optional expansion model is allowed to run.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentTrigger {
    /// Expand beside original retrieval.
    #[default]
    Always,
    /// Expand only after the original ranking is weak or unavailable.
    LowConfidence {
        /// Model-specific threshold; equality does not trigger expansion.
        min_top_rerank: f64,
    },
}

impl IntentTrigger {
    /// Whether the original ranking warrants an extra retrieval pass.
    #[must_use]
    pub fn should_expand(self, score: Option<f64>) -> bool {
        match self {
            Self::Always => true,
            Self::LowConfidence { min_top_rerank } => {
                score.is_none_or(|score| score < min_top_rerank)
            }
        }
    }

    /// Whether a caller-supplied score threshold is finite.
    #[must_use]
    pub fn is_valid(self) -> bool {
        match self {
            Self::Always => true,
            Self::LowConfidence { min_top_rerank } => min_top_rerank.is_finite(),
        }
    }
}

/// Generated retrieval inputs: a query, never citation or answer content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expansion {
    /// Documentation-style hypothetical answer, embedded as a dense query.
    pub passage: String,
    /// Documentation terminology, run as a lexical query.
    pub keywords: String,
}

/// Why an expansion adds no route; each has a stable trace code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpansionFailure {
    /// No expander is configured, or its model refused the call.
    ModelUnavailable,
    /// The expansion did not end before its deadline.
    DeadlineExceeded,
    /// The reply is not the expander's expected shape.
    Malformed,
    /// The passage or the keywords are blank: the model declined.
    Empty,
    /// The reply, the passage or the keywords exceed their bound.
    Oversize,
    /// A constraint word or quantity of the question is missing.
    ProtectedMissing,
    /// A number the question does not state was added.
    AddedNumber,
    /// An identifier of the question is missing from the passage.
    IdentifierMissing,
    /// An identifier the question does not state was added.
    IdentifierAdded,
}

impl ExpansionFailure {
    /// The stable reason recorded as the `intent_expansion` route status.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ModelUnavailable => "intent_model_unavailable",
            Self::DeadlineExceeded => "intent_deadline_exceeded",
            Self::Malformed => "intent_guard_malformed",
            Self::Empty => "intent_guard_empty",
            Self::Oversize => "intent_guard_oversize",
            Self::ProtectedMissing => "intent_guard_protected_missing",
            Self::AddedNumber => "intent_guard_added_number",
            Self::IdentifierMissing => "intent_guard_identifier_missing",
            Self::IdentifierAdded => "intent_guard_identifier_added",
        }
    }
}

/// The future of one expansion.
pub type ExpansionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Expansion, ExpansionFailure>> + Send + 'a>>;

/// Writes retrieval inputs for a question. Search bounds the call with its
/// own deadline and guards the result, so an adapter needs neither.
pub trait QueryExpander: Send + Sync {
    /// Expands `question` into a passage and keywords.
    fn expand<'a>(&'a self, question: &'a Understood) -> ExpansionFuture<'a>;
}
