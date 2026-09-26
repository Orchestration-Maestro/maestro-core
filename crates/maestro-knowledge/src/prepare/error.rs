//! Why a router tokenizer refuses to qualify, or to count.

use maestro_kernel::{
    artifact::Digest,
    gateway::{self, Role},
};
use std::{error, fmt, io, time::Duration};

/// Why a router tokenizer refuses to qualify, or to count.
#[derive(Debug)]
pub enum TokenizerError {
    /// The card is not an embedder's: chunks are budgeted in the tokens of
    /// the embedder that will read them.
    NotAnEmbedder {
        /// The card's digest.
        card: Digest,
        /// The role the card records.
        role: Role,
    },
    /// The embedder does not fit in the router's free room, and counting
    /// never unloads another model (FR-S1-015a).
    Unavailable {
        /// The router's reason.
        reason: String,
    },
    /// Any other refusal of the model port.
    Port(gateway::Error),
    /// The router's ordered IDs for a parity fixture differ from those the
    /// native counter gave it: the first fixture that differs.
    Disagreement {
        /// The fixture's name.
        fixture: String,
        /// The fixture's complete input.
        input: String,
        /// The native counter's IDs.
        native: Vec<u32>,
        /// The router's IDs.
        router: Vec<u32>,
    },
    /// The port did not answer within the deadline: the call was abandoned,
    /// and the next one is answered as usual.
    TimedOut {
        /// The deadline.
        after: Duration,
    },
    /// The parity fixtures built into this crate are not valid JSON of their
    /// shape.
    Fixtures(serde_json::Error),
    /// The thread that runs the port's calls, or its runtime, could not
    /// start.
    Start(io::Error),
    /// The thread that runs the port's calls stopped: a call panicked.
    Stopped,
}

impl TokenizerError {
    /// The refusal a port's `error` means: [`TokenizerError::Unavailable`]
    /// for a model that does not fit in free room, [`TokenizerError::Port`]
    /// for any other.
    pub(super) fn from_port(error: gateway::Error) -> Self {
        match error {
            gateway::Error::Unavailable { reason } => Self::Unavailable { reason },
            other => Self::Port(other),
        }
    }
}

impl fmt::Display for TokenizerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnEmbedder { card, role } => write!(
                formatter,
                "the model card sha256:{} is a {role}'s, and chunks are counted in an \
                 embedder's tokens",
                card.as_str()
            ),
            Self::Unavailable { reason } => write!(
                formatter,
                "the embedder does not fit in the router's free room, and counting never \
                 unloads another model: {reason}"
            ),
            Self::Port(error) => write!(formatter, "the router did not tokenize: {error}"),
            Self::Disagreement {
                fixture,
                native,
                router,
                ..
            } => write!(
                formatter,
                "the router's IDs for the parity fixture {fixture} differ from those of the \
                 native counter: native {native:?}, router {router:?}"
            ),
            Self::TimedOut { after } => {
                write!(formatter, "the router did not tokenize within {after:?}")
            }
            Self::Fixtures(error) => write!(
                formatter,
                "the parity fixtures built into maestro-knowledge are not valid: {error}"
            ),
            Self::Start(error) => {
                write!(formatter, "the tokenizer's thread could not start: {error}")
            }
            Self::Stopped => formatter
                .write_str("the tokenizer's thread stopped: a call to the model port panicked"),
        }
    }
}

impl error::Error for TokenizerError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Port(error) => Some(error),
            Self::Fixtures(error) => Some(error),
            Self::Start(error) => Some(error),
            Self::NotAnEmbedder { .. }
            | Self::Unavailable { .. }
            | Self::Disagreement { .. }
            | Self::TimedOut { .. }
            | Self::Stopped => None,
        }
    }
}
