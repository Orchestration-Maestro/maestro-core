//! The states a chunk set moves through, and the moves it may make.

use std::fmt;

/// Where a chunk set is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkSetState {
    /// Its chunks are being recorded; nothing reads it as a whole.
    Building,
    /// Every eligible revision was prepared and its manifest recorded: the
    /// indexing reads it, and it never changes.
    Complete,
    /// Its counts can no longer be trusted: it is never completed nor
    /// resumed.
    Failed,
}

impl ChunkSetState {
    /// Every state, in lifecycle order.
    const ALL: [Self; 3] = [Self::Building, Self::Complete, Self::Failed];

    /// Whether a chunk set in this state may move to `to`: only from
    /// building, to complete or to failed.
    pub(super) fn may_move_to(self, to: Self) -> bool {
        self == Self::Building && to != Self::Building
    }

    /// Its name, as the `state` column holds it and a refusal gives it.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Building => "building",
            Self::Complete => "complete",
            Self::Failed => "failed",
        }
    }

    /// The state `name` names, if any.
    pub(super) fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.as_str() == name)
    }
}

impl fmt::Display for ChunkSetState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
