//! Why a command stopped short, and the exit code that says so (plan D12):
//! 2 for an input it refused, 1 for an operation that failed.

use crate::presentation::message::Message;
use std::{error::Error, fmt, process::ExitCode};

/// Why a command stopped short.
#[derive(Debug)]
pub(crate) enum Failure {
    /// The input was refused before work: an unknown record, invalid request,
    /// or resource held by another job. Exit code 2, as clap's usage errors.
    Refused(String),
    /// The operation failed because the kernel or file system did. Exit code 1.
    Failed(String),
    /// Typed refusal; machine records and Display retain the English template.
    RefusedMessage(Message),
    /// Typed failure; only the human boundary selects another interface language.
    FailedMessage(Message),
}

impl Failure {
    /// A refusal, for `reason`.
    pub(crate) fn refused(reason: impl fmt::Display) -> Self {
        Self::Refused(reason.to_string())
    }

    /// A failure, for `reason`.
    pub(crate) fn failed(reason: impl fmt::Display) -> Self {
        Self::Failed(reason.to_string())
    }

    /// A refusal, for `error` and its causes.
    pub(crate) fn refused_by(error: &dyn Error) -> Self {
        Self::Refused(chain(error))
    }

    /// A failure, for `error` and its causes.
    pub(crate) fn failed_by(error: &dyn Error) -> Self {
        Self::Failed(chain(error))
    }

    /// A typed refusal without changing the existing string-based constructors.
    pub(crate) const fn refused_message(message: Message) -> Self {
        Self::RefusedMessage(message)
    }

    /// A typed failure without changing English machine diagnostics.
    pub(crate) const fn failed_message(message: Message) -> Self {
        Self::FailedMessage(message)
    }

    /// The exit code that says why: 2 for a refusal, 1 for a failure.
    pub(crate) fn code(&self) -> ExitCode {
        match self {
            Self::Refused(_) | Self::RefusedMessage(_) => ExitCode::from(2),
            Self::Failed(_) | Self::FailedMessage(_) => ExitCode::from(1),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(reason) | Self::Failed(reason) => formatter.write_str(reason),
            Self::RefusedMessage(message) | Self::FailedMessage(message) => message.fmt(formatter),
        }
    }
}

/// The message of `error`, followed by any cause message not already present.
pub(crate) fn chain(error: &dyn Error) -> String {
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
