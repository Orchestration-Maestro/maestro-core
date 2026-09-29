//! The transport-safe echo of the accepted search budget.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The request limits accepted for a search, echoed without exposing a clock
/// instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::min_ident_chars,
    reason = "the maestro-evidence/1 search contract names this limit k"
)]
pub struct RequestBudget {
    /// The maximum final passage count.
    pub k: u32,
    /// The maximum evidence-token budget.
    pub max_tokens: u32,
    /// The accepted deadline duration, in milliseconds.
    pub deadline_ms: u32,
}

impl Default for RequestBudget {
    fn default() -> Self {
        Self {
            k: 10,
            max_tokens: 6000,
            deadline_ms: Self::MAX_DEADLINE_MS,
        }
    }
}

impl RequestBudget {
    /// The longest deadline a search accepts, in milliseconds. It is a
    /// safety cap, not a quality cutoff: a search that loads cold models
    /// under a loaded machine still completes within it.
    pub const MAX_DEADLINE_MS: u32 = 30_000;

    /// The largest evidence budget, `max_tokens`, that a search or an ask
    /// accepts: UTF-8 bytes under the byte counters. The answerer cards'
    /// contexts, 32,768 and 40,960 tokens, hold it with margin. A byte-level
    /// BPE token covers at least one byte, so at worst 24,000 evidence bytes
    /// are 24,000 tokens; with the longest built-in instructions
    /// (`procedure_first`, 1,416 bytes) and a 2,048-token reply that makes
    /// 24,000 + 1,416 + 2,048 = 27,464 tokens, which leaves 5,304 of 32,768
    /// and 13,496 of 40,960 for the question and the chat template. Prose
    /// runs 3 to 4 bytes a token, so 24,000 bytes are typically 6,000 to
    /// 8,000 tokens.
    pub const MAX_EVIDENCE_BUDGET: u32 = 24_000;

    /// Whether this echo is within the search request bounds.
    pub(super) fn validate(&self) -> Result<(), String> {
        if !(1..=50).contains(&self.k) {
            return Err("request budget k must be between 1 and 50".to_owned());
        }
        if !(1..=Self::MAX_EVIDENCE_BUDGET).contains(&self.max_tokens) {
            return Err(format!(
                "request budget max_tokens must be between 1 and {}",
                Self::MAX_EVIDENCE_BUDGET
            ));
        }
        if !(1..=Self::MAX_DEADLINE_MS).contains(&self.deadline_ms) {
            return Err("request budget deadline_ms must be between 1 and 30000".to_owned());
        }
        Ok(())
    }
}
