//! Why a command stopped short, and the exit code that says so (plan D12):
//! 2 for an input it refused, 1 for an operation that failed.

use std::{error::Error, fmt, process::ExitCode};

/// Why a command stopped short.
#[derive(Debug)]
pub(super) enum Failure {
    /// The input was refused, before any work: an unknown collection or job,
    /// a declaration, binding, manifest or configuration that is not what it
    /// must be, or a resource another job holds. Exit code 2, as clap's own
    /// usage errors.
    Refused(String),
    /// The operation failed: the kernel or the file system did. Exit code 1.
    Failed(String),
}

impl Failure {
    /// A refusal, for `reason`.
    pub(super) fn refused(reason: impl fmt::Display) -> Self {
        Self::Refused(reason.to_string())
    }

    /// A failure, for `reason`.
    pub(super) fn failed(reason: impl fmt::Display) -> Self {
        Self::Failed(reason.to_string())
    }

    /// A refusal, for `error` and its causes.
    pub(super) fn refused_by(error: &dyn Error) -> Self {
        Self::Refused(chain(error))
    }

    /// A failure, for `error` and its causes.
    pub(super) fn failed_by(error: &dyn Error) -> Self {
        Self::Failed(chain(error))
    }

    /// The exit code that says why: 2 for a refusal, 1 for a failure.
    pub(super) fn code(&self) -> ExitCode {
        match self {
            Self::Refused(_) => ExitCode::from(2),
            Self::Failed(_) => ExitCode::from(1),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(reason) | Self::Failed(reason) => formatter.write_str(reason),
        }
    }
}

/// The message of `error`, followed by that of each of its causes it does
/// not give already: some errors of the kernel repeat their cause's message,
/// others leave it to their source.
pub(super) fn chain(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut cause = error.source();
    while let Some(each) = cause {
        let text = each.to_string();
        if !message.contains(&text) {
            message.push_str(": ");
            message.push_str(&text);
        }
        cause = each.source();
    }
    message
}
