//! A check of the machine: what it looked at, and whether it passed,
//! saying what it saw, or failed, saying what is wrong and the next action
//! that fixes it.

/// What a check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Outcome {
    /// It passed, and saw this.
    Passed(String),
    /// It failed.
    Failed {
        /// What is wrong.
        problem: String,
        /// The next action, which fixes it.
        next: String,
    },
}

/// One check of the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Check {
    /// Its name, such as `database`.
    pub(super) name: &'static str,
    /// What it looked at: a file, a directory, an address or a role.
    pub(super) target: String,
    /// What it found.
    pub(super) outcome: Outcome,
}

impl Check {
    /// The check `name` of `target`, which passed, having seen `detail`.
    pub(super) fn passed(name: &'static str, target: &str, detail: impl Into<String>) -> Self {
        Self {
            name,
            target: target.to_owned(),
            outcome: Outcome::Passed(detail.into()),
        }
    }

    /// The check `name` of `target`, which failed on `problem`, fixed by
    /// `next`.
    pub(super) fn failed(
        name: &'static str,
        target: &str,
        problem: impl Into<String>,
        next: impl Into<String>,
    ) -> Self {
        Self {
            name,
            target: target.to_owned(),
            outcome: Outcome::Failed {
                problem: problem.into(),
                next: next.into(),
            },
        }
    }

    /// What it saw when it passed, or what is wrong when it failed.
    pub(super) fn detail(&self) -> &str {
        match &self.outcome {
            Outcome::Passed(detail)
            | Outcome::Failed {
                problem: detail, ..
            } => detail,
        }
    }

    /// The next action, when it failed.
    pub(super) fn next(&self) -> Option<&str> {
        match &self.outcome {
            Outcome::Passed(_) => None,
            Outcome::Failed { next, .. } => Some(next),
        }
    }
}
