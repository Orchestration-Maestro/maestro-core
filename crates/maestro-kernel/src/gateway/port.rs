//! The model port: the calls every way of reaching a model answers, each
//! bound to a model card, and the refusals they share.

use super::{
    card::{CardFields, ModelCard, Role},
    card_v2::{Capability, ControlValue, Sampling, SamplingParameters},
};
use crate::{
    artifact::Digest,
    facts::{EntityName, Object},
    vocabulary::Predicate,
};
use serde::Serialize;
use serde_json::{Map, Number, Value, json};
use std::{
    collections::BTreeMap,
    error, fmt,
    future::Future,
    num::{NonZeroU32, NonZeroUsize},
};

/// One source text submitted for extraction. The gateway fixes authority,
/// vocabulary, sampling, template controls and output ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractRequest {
    /// The source text, kept private so callers cannot add authority fields.
    pub(super) input: String,
}

impl ExtractRequest {
    /// Makes a request from nonempty source text.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidRequest`] when the input is empty or whitespace.
    pub fn new(input: &str) -> Result<Self, Error> {
        if input.trim().is_empty() {
            return Err(Error::InvalidRequest {
                reason: "extraction input must not be empty".to_owned(),
            });
        }
        Ok(Self {
            input: input.to_owned(),
        })
    }
}

/// A typed extraction suggestion with no source authority or approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The typed subject.
    pub subject: EntityName,
    /// A claimable predicate from the shared closed vocabulary.
    pub predicate: Predicate,
    /// The predicate's typed entity or literal object.
    pub object: Object,
}

/// The calls a model answers, each bound to the card of the model that
/// answers it: an embedding needs an embedder's card, a reranking a
/// reranker's and a chat an answerer's, while any card tokenizes. Each call
/// also names the [`Room`] its model may be loaded into.
pub trait ModelPort {
    /// Readies the card's model before a call, in `room`: a router checks
    /// the card and loads its model here, also after unloading it while
    /// idle, so that a caller can bound this setup apart from the call. A
    /// port with no setup has nothing to do.
    fn prepare(
        &self,
        card: &ModelCard,
        room: Room,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let _ = (card, room);
        async { Ok(()) }
    }

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

    /// Render the complete chat template, including the assistant generation prefix.
    /// Adapters without template rendering refuse rather than approximate token counts.
    fn render_chat(
        &self,
        card: &ModelCard,
        room: Room,
        request: &ChatRequest,
    ) -> impl Future<Output = Result<String, Error>> + Send {
        let _ = (card, room, request);
        async { Err(Error::Unsupported) }
    }

    /// One buffered reply to an explicitly bounded prompt.
    fn chat(
        &self,
        card: &ModelCard,
        room: Room,
        request: &ChatRequest,
    ) -> impl Future<Output = Result<String, Error>> + Send;

    /// One bounded, closed-schema extraction from an Extractor card. A
    /// successful response is a whole set of typed candidates or an error.
    fn extract(
        &self,
        card: &ModelCard,
        request: &ExtractRequest,
    ) -> impl Future<Output = Result<Vec<Candidate>, Error>> + Send {
        let _ = (card, request);
        async { Err(Error::Unsupported) }
    }
}

/// Where a call lets the router load its model when the model is not loaded
/// yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Room {
    /// Only into free room: the router refuses the call, [`Error::Unavailable`],
    /// rather than unload another model. Search and local ask calls use it, so
    /// neither unloads another model (FR-S1-015a).
    Free,
    /// Anywhere the router can make room, unloading idle models as it does for
    /// any request; local ask does not use it.
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
    /// This adapter does not support the requested model-port operation.
    Unsupported,
    /// The caller's model request violates its bounded request contract.
    InvalidRequest {
        /// How the request differs.
        reason: String,
    },
    /// The router answered with a redirect, which this gateway never follows:
    /// a followed 307 or 308 would send the call's body, private text
    /// included, wherever the redirect points.
    Redirected {
        /// The redirect's HTTP status.
        status: u16,
    },
    /// The model's answer is not what the call expects.
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
            Self::Unsupported => formatter.write_str("this model-port operation is unsupported"),
            Self::InvalidRequest { reason } => write!(formatter, "invalid model request: {reason}"),
            Self::Redirected { status } => write!(
                formatter,
                "the router answered with redirect {status}, which this gateway never follows"
            ),
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

/// The kernel's safety ceiling on the tokens of one chat reply, whatever the
/// caller or the answerer's card asks for. Two bounds set it. The chat
/// deadline, 20 s: a cold answerer loaded and thought through 1,024 tokens in
/// 6.5 s, so 2,048 take at most twice that, while 4,096 could pass the
/// deadline. The router client's 32,768-byte content cap: 2,048 tokens fit it
/// at 16 bytes per token, four times what text averages.
pub const MAX_CHAT_OUTPUT_TOKENS: u32 = 2048;

/// The kernel's ceiling on the tokens of one constrained extraction, whatever
/// the extractor's card declares. Its content bound is 16 bytes per token.
pub const MAX_EXTRACT_OUTPUT_TOKENS: u32 = 1024;

/// The reply cap of an answerer whose card declares no output limit.
pub const DEFAULT_CHAT_OUTPUT_TOKENS: u32 = 1024;

/// The most tokens one chat reply of `card`'s model may generate: the
/// smallest of the caller's `requested` cap, the card's declared output
/// limit and [`MAX_CHAT_OUTPUT_TOKENS`]. A card that declares no limit
/// stands for [`DEFAULT_CHAT_OUTPUT_TOKENS`] when the caller sets none.
#[must_use]
pub fn reply_cap(card: &ModelCard, requested: Option<u32>) -> u32 {
    let declared = card.fields().limits.output_tokens.map(NonZeroU32::get);
    let wanted = requested.or(declared).unwrap_or(DEFAULT_CHAT_OUTPUT_TOKENS);
    declared
        .map_or(wanted, |declared| wanted.min(declared))
        .min(MAX_CHAT_OUTPUT_TOKENS)
}

/// One non-streaming chat prompt with bounded output and template controls.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatRequest {
    /// The messages sent to the answerer.
    pub messages: Vec<Message>,
    /// Maximum generated tokens, at most [`MAX_CHAT_OUTPUT_TOKENS`]; [`reply_cap`]
    /// gives an answerer's.
    pub max_output_tokens: u32,
    /// Exact supported chat-template controls, not arbitrary router options.
    pub chat_template_kwargs: BTreeMap<String, ControlValue>,
}

impl ChatRequest {
    /// Makes a bounded request with no template controls.
    #[must_use]
    pub fn new(messages: Vec<Message>, max_output_tokens: u32) -> Self {
        Self {
            messages,
            max_output_tokens,
            chat_template_kwargs: BTreeMap::new(),
        }
    }

    /// Refuses controls outside local limits or different from a v2 card.
    pub(super) fn validate<'a>(
        &self,
        card: &'a ModelCard,
    ) -> Result<Option<&'a SamplingParameters>, Error> {
        require(card, Role::Answerer)?;
        let sampling = chat_sampling(
            card.identity()
                .map(|identity| &identity.invocation.sampling),
        )?;
        if !(1..=MAX_CHAT_OUTPUT_TOKENS).contains(&self.max_output_tokens) {
            return Err(Error::InvalidRequest {
                reason: format!(
                    "output limit must be between 1 and {MAX_CHAT_OUTPUT_TOKENS} tokens"
                ),
            });
        }
        if card
            .fields()
            .limits
            .output_tokens
            .is_some_and(|limit| self.max_output_tokens > limit.get())
        {
            return Err(invalid_request("output limit exceeds the answerer's card"));
        }
        for (name, value) in &self.chat_template_kwargs {
            if name.trim().is_empty()
                || matches!(value, ControlValue::Number(number) if !number.is_finite())
            {
                return Err(invalid_request(
                    "template controls must have names and finite values",
                ));
            }
        }
        if let Some(identity) = card.identity() {
            let supported = match &identity.invocation.reasoning {
                Capability::Supported(controls) => controls == &self.chat_template_kwargs,
                Capability::Unsupported => self.chat_template_kwargs.is_empty(),
                Capability::NotApplicable => false,
            };
            if !supported {
                return Err(invalid_request(
                    "template controls differ from the answerer's card",
                ));
            }
        }
        Ok(sampling)
    }

    /// The template controls in their untagged llama.cpp wire representation.
    pub(super) fn template_values(&self) -> Result<Value, Error> {
        self.chat_template_kwargs
            .iter()
            .map(|(name, value)| Ok((name.clone(), control_value(value)?)))
            .collect::<Result<Map<_, _>, Error>>()
            .map(Value::Object)
    }
}

/// Adds every sampling field a card records to a router request `body`.
pub(super) fn sampling_fields(body: &mut Map<String, Value>, sampling: &SamplingParameters) {
    for (field, value) in [
        ("temperature", json!(sampling.temperature)),
        ("top_p", json!(sampling.top_p)),
        ("top_k", json!(sampling.top_k)),
        ("min_p", json!(sampling.min_p)),
        ("typical_p", json!(sampling.typical_p)),
        ("repeat_penalty", json!(sampling.repeat_penalty)),
        ("frequency_penalty", json!(sampling.frequency_penalty)),
        ("presence_penalty", json!(sampling.presence_penalty)),
    ] {
        body.insert(field.to_owned(), value);
    }
    if let Some(seed) = sampling.seed {
        body.insert("seed".to_owned(), json!(seed));
    }
}

/// Converts a typed model-card control to the router's raw template value.
pub(super) fn control_value(value: &ControlValue) -> Result<Value, Error> {
    match value {
        ControlValue::Boolean(value) => Ok(Value::Bool(*value)),
        ControlValue::Integer(value) => Ok(Value::from(*value)),
        ControlValue::Number(value) => Number::from_f64(*value)
            .map(Value::Number)
            .ok_or_else(|| invalid_request("template control is not finite")),
        ControlValue::Text(value) => Ok(Value::String(value.clone())),
    }
}

/// Sampling values recorded by a v2 answerer; a v1 card records none.
pub(super) fn chat_sampling(
    sampling: Option<&Sampling>,
) -> Result<Option<&SamplingParameters>, Error> {
    match sampling {
        Some(Sampling::Configured(settings)) => Ok(Some(settings)),
        Some(Sampling::NotApplicable) => Err(invalid_request(
            "the v2 answerer's card must record generation sampling",
        )),
        None => Ok(None),
    }
}

/// A sanitized refusal for an invalid caller-supplied chat request.
fn invalid_request(reason: &'static str) -> Error {
    Error::InvalidRequest {
        reason: reason.to_owned(),
    }
}
