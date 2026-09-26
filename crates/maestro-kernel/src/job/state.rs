//! The states a job moves through, and the moves it may make.

use std::fmt;

/// Where a job is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    /// Submitted: no one has taken its lease yet.
    Queued,
    /// Its holder works on it under a lease.
    Running,
    /// It did what it was submitted for: final.
    Succeeded,
    /// It stopped short of it: final; its key starts a new attempt.
    Failed,
    /// It was withdrawn, queued or running: final; its key starts a new
    /// attempt.
    Cancelled,
}

/// Every move a job may make, from one state to another: forward only.
const MOVES: [(JobState, JobState); 5] = [
    (JobState::Queued, JobState::Running),
    (JobState::Queued, JobState::Cancelled),
    (JobState::Running, JobState::Succeeded),
    (JobState::Running, JobState::Failed),
    (JobState::Running, JobState::Cancelled),
];

impl JobState {
    /// Every state, in the order a job meets them.
    const ALL: [Self; 5] = [
        Self::Queued,
        Self::Running,
        Self::Succeeded,
        Self::Failed,
        Self::Cancelled,
    ];

    /// Whether a job in this state may move to `to`.
    pub(super) fn may_move_to(self, to: Self) -> bool {
        MOVES.contains(&(self, to))
    }

    /// Its name, as the `state` column holds it and a refusal gives it.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// The state `name` names, if any.
    pub(super) fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.as_str() == name)
    }
}

impl fmt::Display for JobState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
