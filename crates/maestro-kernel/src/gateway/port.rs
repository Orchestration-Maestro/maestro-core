//! The model port: the calls every way of reaching a model answers, each
//! bound to a model card, and the refusals they share.

use super::card::{CardFields, ModelCard, Role};
use crate::artifact::Digest;
use serde::Serialize;
use std::{error, fmt, future::Future, num::NonZeroUsize};

/// The calls a model answers, each bound to the card of the model that
/// answers it: an embedding needs an embedder's card, a reranking a
/// reranker's and a chat an answerer's, while any card tokenizes. Each call
/// also names the [`Room`] its model may be loaded into.
pub trait ModelPort {
    /// One vector per input, in the order of the inputs, each of the card's
    /// dimensions. No input needs no call.
    fn embed(
        &self,
        card: &ModelCard,
        room: Room,
        inputs: &[String],
    ) -> impl Future<Output = Result<Vec<Vec<f32>>, Error>> + Send;

    /// One relevance score per document, in the order of the documents: the
    /// higher, the more relevant to `query`. No document needs no call.
    fn rerank(
        &self,
        card: &ModelCard,
        room: Room,
        query: &str,
        documents: &[String],
    ) -> impl Future<Output = Result<Vec<f64>, Error>> + Send;

    /// The token IDs of `text`, in order, as the card's model counts them.
    fn tokenize(
        &self,
        card: &ModelCard,
        room: Room,
        text: &str,
    ) -> impl Future<Output = Result<Vec<u32>, Error>> + Send;

    /// The reply to `messages`, the last of which is the question.
    fn chat(
        &self,
        card: &ModelCard,
        room: Room,
        messages: &[Message],
    ) -> impl Future<Output = Result<String, Error>> + Send;
}

/// Where a call lets the router load its model when the model is not loaded
/// yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Room {
    /// Only into free room: the router refuses the call, [`Error::Unavailable`],
    /// rather than unload another model. Search calls use it, so that a search
    /// never unloads a chat model (FR-S1-015a).
    Free,
    /// Anywhere the router can make room, unloading idle models as it does for
    /// any request. `ask`'s answerer uses it.
    Any,
}

/// Who says a message of a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    /// The instructions the model follows.
    System,
    /// The one asking, and the evidence given with the question.
    User,
    /// The model's own earlier replies.
    Assistant,
}

/// One message of a chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    /// Who says it.
    #[serde(rename = "role")]
    pub speaker: Speaker,
    /// What is said.
    pub content: String,
}

/// Why a call through the port did not answer.
#[derive(Debug)]
pub enum Error {
    /// The card's role cannot answer the call, which is never made.
    WrongRole {
        /// The card's digest.
        card: Digest,
        /// The role the card records.
        role: Role,
        /// The role the call needs.
        needed: Role,
    },
    /// The model's server does not report what the card records, its build
    /// and, where the card records one, its chat template, as `/props` gives
    /// them: the card is refused before any call.
    CardMismatch {
        /// The card's digest.
        card: Digest,
        /// The `/props` field that differs: `build_info` or `chat_template`.
        property: &'static str,
        /// What the card records.
        recorded: String,
        /// What the server reports; for the template, the digest of it, or
        /// `none` when the server reports no template.
        reported: String,
    },
    /// The router cannot make room for the model in the room the call names
    /// (`503 insufficient_room`): the route is unavailable, for the reason
    /// the router gives.
    Unavailable {
        /// The router's message.
        reason: String,
    },
    /// The router does not know the room this gateway asked for
    /// (`400 unknown_room`): a bug in the gateway, never in the call.
    UnknownRoom {
        /// The router's message.
        message: String,
    },
    /// Any other refusal, by the router or by the model's server.
    Refused {
        /// The HTTP status.
        status: u16,
        /// The refusal's code, when it has one in words.
        code: Option<String>,
        /// The refusal's message, or the whole answer when it is no JSON
        /// refusal.
        message: String,
    },
    /// The answer is not what the call expects.
    InvalidAnswer {
        /// How it differs.
        reason: String,
    },
    /// The router could not be reached, or the exchange broke off.
    Transport(reqwest::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongRole { card, role, needed } => write!(
                formatter,
                "the model card sha256:{} is a {role}'s, and this call needs a {needed}'s",
                card.as_str()
            ),
            Self::CardMismatch {
                card,
                property,
                recorded,
                reported,
            } => write!(
                formatter,
                "the model card sha256:{} records {property} {recorded:?}, but the model's \
                 server reports {reported:?}",
                card.as_str()
            ),
            Self::Unavailable { reason } => write!(formatter, "the model is unavailable: {reason}"),
            Self::UnknownRoom { message } => write!(
                formatter,
                "the router refused the room this gateway asked for: {message}"
            ),
            Self::Refused {
                status,
                code: Some(code),
                message,
            } => write!(formatter, "refused with {status} {code}: {message}"),
            Self::Refused {
                status,
                code: None,
                message,
            } => write!(formatter, "refused with {status}: {message}"),
            Self::InvalidAnswer { reason } => {
                write!(
                    formatter,
                    "the model's answer is not what the call expects: {reason}"
                )
            }
            Self::Transport(_) => {
                formatter.write_str("the router could not be reached, or the exchange broke off")
            }
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Transport(source) => Some(source),
            _ => None,
        }
    }
}

/// Refuses `card` unless it records `needed`.
pub(super) fn require(card: &ModelCard, needed: Role) -> Result<(), Error> {
    let role = card.fields().role;
    if role == needed {
        Ok(())
    } else {
        Err(Error::WrongRole {
            card: card.digest().clone(),
            role,
            needed,
        })
    }
}

/// The dimensions of an embedder's card; any other card is refused.
pub(super) fn embedder_dimensions(card: &ModelCard) -> Result<NonZeroUsize, Error> {
    match card.fields() {
        CardFields {
            role: Role::Embedder,
            dimensions: Some(dimensions),
            ..
        } => Ok(*dimensions),
        fields => Err(Error::WrongRole {
            card: card.digest().clone(),
            role: fields.role,
            needed: Role::Embedder,
        }),
    }
}
