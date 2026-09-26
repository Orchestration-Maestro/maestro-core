//! The states a generation moves through, and the one move each allows.

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
}

impl GenerationState {
    /// Every state, in lifecycle order.
    const ALL: [Self; 4] = [
        Self::Building,
        Self::Verified,
        Self::Published,
        Self::Retired,
    ];

    /// The one state it may move to, if any.
    pub(super) fn next(self) -> Option<Self> {
        match self {
            Self::Building => Some(Self::Verified),
            Self::Verified => Some(Self::Published),
            Self::Published => Some(Self::Retired),
            Self::Retired => None,
        }
    }

    /// Its name, as the `state` column holds it and a refusal gives it.
    fn as_str(self) -> &'static str {
        match self {
            Self::Building => "building",
            Self::Verified => "verified",
            Self::Published => "published",
            Self::Retired => "retired",
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
