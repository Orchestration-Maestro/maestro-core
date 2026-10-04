//! Versioned transition data; implementations never create dependency cycles through records.
use serde::{Deserialize, Serialize};

/// An immutable old-to-new transition; host adapters retain entry-level ownership.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReplacementPlan {
    /// Versioned write-ahead shape, not installation authority.
    pub(super) schema: String,
    /// Validated root-relative leaf.
    pub(super) path: String,
    /// Exact old bytes, or an absent target.
    pub(super) old: Option<Vec<u8>>,
    /// Exact caller-computed new bytes.
    pub(super) new: Vec<u8>,
    /// Digest binding the complete transition.
    pub(super) id: String,
}
