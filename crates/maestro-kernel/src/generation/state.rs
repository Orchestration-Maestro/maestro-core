//! The states a generation moves through, and the moves it may make.

use std::fmt;

/// Where a generation is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationState {
    /// Its points are being written; nothing reads it.
    Building,
    /// Its checks passed; it waits to be published.
    Verified,
    /// Searches read it: at most one per collection.
    Published,
    /// A newer generation replaced it, or it was withdrawn.
    Retired,
    /// It failed before it was published: it is never published, nor resumed.
    Failed,
}

/// Every move a generation may make, from one state to another: forward one
/// state at a time, or to failed before it is published.
const MOVES: [(GenerationState, GenerationState); 5] = [
    (GenerationState::Building, GenerationState::Verified),
    (GenerationState::Building, GenerationState::Failed),
    (GenerationState::Verified, GenerationState::Published),
    (GenerationState::Verified, GenerationState::Failed),
    (GenerationState::Published, GenerationState::Retired),
];

impl GenerationState {
    /// Every state, in lifecycle order.
    const ALL: [Self; 5] = [
        Self::Building,
        Self::Verified,
        Self::Published,
        Self::Retired,
        Self::Failed,
    ];

    /// Whether a generation in this state may move to `to`.
    pub(super) fn may_move_to(self, to: Self) -> bool {
        MOVES.contains(&(self, to))
    }

    /// Its name, as the `state` column holds it and a refusal gives it.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Building => "building",
            Self::Verified => "verified",
            Self::Published => "published",
            Self::Retired => "retired",
            Self::Failed => "failed",
        }
    }

    /// The state `name` names, if any.
    pub(super) fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.as_str() == name)
    }
}

impl fmt::Display for GenerationState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
