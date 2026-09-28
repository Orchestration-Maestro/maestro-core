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
            deadline_ms: 1500,
        }
    }
}

impl RequestBudget {
    /// Whether this echo is within the search request bounds.
    pub(super) fn validate(&self) -> Result<(), String> {
        if !(1..=50).contains(&self.k) {
            return Err("request budget k must be between 1 and 50".to_owned());
        }
        if !(1..=12_000).contains(&self.max_tokens) {
            return Err("request budget max_tokens must be between 1 and 12000".to_owned());
        }
        if !(1..=10_000).contains(&self.deadline_ms) {
            return Err("request budget deadline_ms must be between 1 and 10000".to_owned());
        }
        Ok(())
    }
}
