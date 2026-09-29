//! Typed ask requests, results, refusals, and trusted dependencies.

use super::presentation::{Presentation, Tone};
use crate::search::{
    SearchContext, SearchError,
    evidence::{Anchor, EvidenceError},
};
use maestro_kernel::{
    evidence::{RequestBudget, RouteStatus},
    gateway::{Error as GatewayError, MAX_CHAT_OUTPUT_TOKENS, ModelCard},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, error, fmt, time::Duration};

/// Default local answerer router entry.
pub const DEFAULT_MODEL: &str = "qwen3-4b";
/// Maximum time allowed for each buffered chat call, the answerer's load
/// included. A safety cap: a cold answerer loaded and thought through 1024
/// tokens in 6.5 s, and a load alone took up to 5.3 s on a busy machine.
pub const CHAT_DEADLINE: Duration = Duration::from_secs(20);
/// Number of closest passages retained in a refusal.
pub(super) const CLOSEST_LIMIT: usize = 3;
/// Result schema identifier.
pub(super) const ANSWER_SCHEMA: &str = "maestro-answer/1";

/// Language used for host-owned response metadata when detection is confident.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ResponseLanguage {
    /// English response text.
    English,
    /// French response text.
    French,
}

impl ResponseLanguage {
    /// The short language code used in the public answer.
    pub(super) fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::French => "fr",
        }
    }
}

/// Bounded limits accepted for one ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[expect(
    clippy::min_ident_chars,
    reason = "the shared maestro-evidence search-budget contract names this limit k"
)]
pub struct AskBudget {
    /// Maximum passages assembled from search.
    pub k: u32,
    /// UTF-8-byte evidence budget used by the deliberately uncalibrated flow.
    pub max_tokens: u32,
    /// Search and evidence-assembly deadline in milliseconds. Its default,
    /// 30 s, is a safety cap: search loads a cold embedder and a cold
    /// reranker (each load measured 2.4-5.3 s on a busy machine) and still
    /// runs dense and rerank.
    pub search_deadline_ms: u32,
    /// Maximum generated tokens per chat call; absent, the answerer card's
    /// declared output limit, or 1,024 when it declares none. Every call is
    /// capped at 2,048.
    pub output_tokens: Option<u32>,
}

impl Default for AskBudget {
    fn default() -> Self {
        Self {
            k: 5,
            max_tokens: 6000,
            search_deadline_ms: RequestBudget::MAX_DEADLINE_MS,
            output_tokens: None,
        }
    }
}

impl From<AskBudget> for RequestBudget {
    /// The search bounds of an ask: its passages, evidence budget and search
    /// deadline.
    fn from(budget: AskBudget) -> Self {
        Self {
            k: budget.k,
            max_tokens: budget.max_tokens,
            deadline_ms: budget.search_deadline_ms,
        }
    }
}

impl AskBudget {
    /// Whether every bound is within what `ask` accepts: 1 to 50 passages,
    /// 1 to [`RequestBudget::MAX_EVIDENCE_BUDGET`] evidence bytes, 1 to
    /// 30,000 ms of search and, when set, 1 to [`MAX_CHAT_OUTPUT_TOKENS`]
    /// output tokens.
    #[must_use]
    pub fn is_within_limits(&self) -> bool {
        (1..=50).contains(&self.k)
            && (1..=RequestBudget::MAX_EVIDENCE_BUDGET).contains(&self.max_tokens)
            && (1..=RequestBudget::MAX_DEADLINE_MS).contains(&self.search_deadline_ms)
            && self
                .output_tokens
                .is_none_or(|tokens| (1..=MAX_CHAT_OUTPUT_TOKENS).contains(&tokens))
    }
}

/// The version of the answer prompt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptVersion {
    /// The first prompt: a marker after each supported sentence.
    V1,
    /// After each sentence, the passages that state it, the specific one
    /// over a general one, and `NOT_FOUND` unless the passages answer
    /// directly. The default: the T037 ladder measured it best.
    #[default]
    V2,
    /// Selects the directly applicable procedure before contextual passages.
    #[serde(rename = "procedure_first")]
    ProcedureFirst,
}

impl PromptVersion {
    /// Its name, as manifests and reports write it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::V1 => "v1",
            Self::V2 => "v2",
            Self::ProcedureFirst => "procedure_first",
        }
    }
}

/// The slot of a [`PromptText`]'s user text that the question and evidence
/// data fill.
pub const DATA_SLOT: &str = "{data}";

/// An answer prompt's own texts, as a ladder rung's prompt file holds them;
/// the user text holds [`DATA_SLOT`] exactly once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptText {
    /// The system instruction.
    system: String,
    /// The user instruction, holding the data slot once.
    user: String,
}

impl PromptText {
    /// The prompt of `system` and `user`.
    ///
    /// # Errors
    ///
    /// [`AskError::InvalidRequest`] when `user` does not hold [`DATA_SLOT`]
    /// exactly once.
    pub fn new(system: String, user: String) -> Result<Self, AskError> {
        if user.matches(DATA_SLOT).count() != 1 {
            return Err(AskError::InvalidRequest(
                "the prompt's user text must hold the {data} slot exactly once",
            ));
        }
        Ok(Self { system, user })
    }

    /// The system instruction.
    #[must_use]
    pub fn system(&self) -> &str {
        &self.system
    }

    /// The user instruction, holding the data slot once.
    #[must_use]
    pub fn user(&self) -> &str {
        &self.user
    }
}

/// The prompt the answerer is given: a version's constant texts, those
/// texts presented in a session's language and tone, or a ladder rung's own.
/// The host checks of a reply are the same for all three.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnswerPrompt {
    /// The constant texts of a prompt version.
    Version(PromptVersion),
    /// A version's texts in a session's language and tone, as the CLI and
    /// the MCP server ask; an evaluation never uses it.
    Presented {
        /// The prompt version.
        version: PromptVersion,
        /// The session's language and tone.
        presentation: Presentation,
    },
    /// A ladder rung's own texts.
    Text(PromptText),
}

impl AnswerPrompt {
    /// The language and tone it answers in: the question's language and the
    /// normal tone unless it is presented.
    #[must_use]
    pub fn presentation(&self) -> &Presentation {
        /// The presentation of an unpresented prompt.
        static UNPRESENTED: Presentation = Presentation {
            language: None,
            tone: Tone::Normal,
        };
        match self {
            Self::Presented { presentation, .. } => presentation,
            Self::Version(_) | Self::Text(_) => &UNPRESENTED,
        }
    }
}

impl From<PromptVersion> for AnswerPrompt {
    fn from(version: PromptVersion) -> Self {
        Self::Version(version)
    }
}

/// One bounded ask request shared by CLI and MCP.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AskRequest {
    /// Visible collection to search.
    pub collection: String,
    /// Original user question.
    pub question: String,
    /// Registered answerer router entry; omitted requests use `qwen3-4b`.
    #[serde(default = "default_model")]
    pub model: String,
    /// Exact version filter, when supplied.
    #[serde(default)]
    pub version: Option<String>,
    /// Accepted passage, evidence, search and output bounds.
    #[serde(default)]
    pub budget: AskBudget,
}

/// Supplies the bounded local answerer when a request omits `model`.
fn default_model() -> String {
    DEFAULT_MODEL.to_owned()
}

/// One answerer card loaded from the collection's scoped model registry.
#[derive(Debug, Clone)]
pub struct RegisteredAnswerer {
    /// The immutable registration ID, not a model-supplied value.
    pub id: String,
    /// The registered answerer's immutable card.
    pub card: ModelCard,
}

/// Trusted dependencies for one ask operation.
#[derive(Debug)]
pub struct AnswerContext<'a, P> {
    /// Shared bounded retrieval context.
    pub search: SearchContext<'a, P>,
    /// The gateway used for search and answer generation.
    pub port: &'a P,
    /// The registered answerer for `AskRequest::model`, when available.
    pub answerer: Option<RegisteredAnswerer>,
}

/// A host-owned refusal category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    /// The available passages do not answer the question.
    NotFound,
    /// The generated response failed deterministic support checks.
    Unsupported,
    /// No registered answerer is available for the requested router entry.
    AnswererUnavailable,
    /// Search returned no matching passage.
    NoEvidence,
}

/// A checked refusal with a host-written reason.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnswerRefusal {
    /// Stable public refusal code.
    pub code: RefusalCode,
    /// Host-owned message in the checked response language.
    pub message: String,
}

/// A source-resolved citation copied from an assembled passage.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnswerCitation {
    /// The evidence bundle's citation number.
    pub n: u32,
    /// One source chunk supporting this passage.
    pub chunk_id: String,
    /// The section identity, when present.
    pub section_id: Option<String>,
    /// Source-owned reference, never model output.
    pub source_ref: String,
    /// Source-owned title.
    pub title: String,
    /// Source-owned heading path.
    pub section_path: Vec<String>,
    /// Half-open byte span in the original source.
    pub span: [usize; 2],
}

/// A locally rendered answer or an intentional evidence/model refusal.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    /// Result schema identifier.
    pub schema: String,
    /// Collection searched.
    pub collection: String,
    /// Pinned generation searched.
    pub generation: i64,
    /// Original question.
    pub question: String,
    /// Checked response language code.
    pub lang: String,
    /// Validated answer text; empty on full refusal.
    pub answer: String,
    /// Source metadata for cited passage numbers only.
    pub citations: Vec<AnswerCitation>,
    /// Host-resolved answerer identity.
    pub model: AnswerModel,
    /// True until a ladder run passes every M1 floor; T037 closed with two
    /// answer floors unmet.
    pub uncalibrated: bool,
    /// Host-owned safe refusal, when the evidence or answerer is insufficient.
    pub refusal: Option<AnswerRefusal>,
    /// Host-selected closest passage metadata; never supporting citations.
    pub closest: Vec<AnswerCitation>,
    /// Why each rejected reply failed its checks, for a local explanation
    /// only: never serialized.
    #[serde(skip)]
    pub rejections: Vec<Rejection>,
    /// The status of each stage of the search the answer was assembled from,
    /// for evaluation only: never serialized.
    #[serde(skip)]
    pub routes: BTreeMap<String, RouteStatus>,
    /// The anchors of the bundle the answer was given, in passage order, for
    /// evaluation only: never serialized.
    #[serde(skip)]
    pub delivered: Vec<Anchor>,
    /// The most tokens each chat reply could generate, absent when the ask
    /// never reached the answerer, for evaluation only: never serialized.
    #[serde(skip)]
    pub reply_cap: Option<u32>,
}

/// One answerer reply the host checks rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection {
    /// The attempt, from 1.
    pub attempt: u8,
    /// The stable code of the failed check, such as `unsupported_literal`.
    pub check: &'static str,
    /// The reply's tokens that failed the check, or the gateway's reason.
    pub tokens: Vec<String>,
}

/// The exact card selected from the collection registry for this call.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnswerModel {
    /// Router catalog entry.
    pub router_entry: String,
    /// Registry card ID; absent when no matching answerer is available.
    pub card_id: Option<String>,
}

/// Why the ask request or its bounded dependencies could not complete safely.
#[derive(Debug)]
pub enum AskError {
    /// Request validation failed before model work.
    InvalidRequest(&'static str),
    /// Search admission or route execution failed.
    Search(SearchError),
    /// Evidence assembly or source integrity failed.
    Evidence(EvidenceError),
    /// The bounded router chat call failed.
    Backend(GatewayError),
    /// A chat call exceeded its per-call deadline.
    TimedOut,
    /// An assembled citation lacks its source chunk identity.
    EvidenceIntegrity,
    /// The local answer prompt could not be serialized.
    Json(serde_json::Error),
}

impl fmt::Display for AskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(reason) => write!(formatter, "invalid ask request: {reason}"),
            Self::Search(_) => formatter.write_str("knowledge search could not complete"),
            Self::Evidence(_) => formatter.write_str("evidence could not be verified"),
            Self::Backend(_) => formatter.write_str("the answerer is unavailable"),
            Self::TimedOut => formatter.write_str("the answerer exceeded its 20-second deadline"),
            Self::EvidenceIntegrity => {
                formatter.write_str("the assembled passage has no source chunk identity")
            }
            Self::Json(_) => formatter.write_str("the bounded answer prompt could not be built"),
        }
    }
}

impl error::Error for AskError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Search(error) => Some(error),
            Self::Evidence(error) => Some(error),
            Self::Backend(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::InvalidRequest(_) | Self::TimedOut | Self::EvidenceIntegrity => None,
        }
    }
}
