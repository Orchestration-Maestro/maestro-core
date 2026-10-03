//! One bounded local drafting call; model text never grants evidence authority.
use super::{Original, Stage, check_labels, label_format::Label, label_validation::safe_id};
use crate::suite::Suite;
use maestro_kernel::{
    artifact::Digest,
    gateway::{
        ChatRequest, Error as GatewayError, Message, ModelCard, ModelPort, Role, Room, Speaker,
        card_v2::Capability,
    },
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, convert::Infallible, error::Error, fmt, time::Duration};
use tokio::time::timeout;

/// A source and byte window selected by the approved inventory, never by the model.
#[derive(Clone)]
pub struct DraftWindow {
    /// Host-assigned question/window ID.
    pub id: String,
    /// Source authority reference.
    pub source_ref: String,
    /// Pinned revision identity.
    pub revision_id: String,
    /// Digest of all original source bytes.
    pub original: Digest,
    /// Only this half-open span may be sent to inference or cited.
    pub span: [usize; 2],
    /// Private original source, loaded only after scope admission.
    pub source: String,
}

/// Private result envelope, persisted only after source revalidation.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftCandidate {
    /// Host-assigned item ID.
    pub id: String,
    /// Independent family, unique across accepted drafts.
    pub family: String,
    /// Strict single-question suite JSONL.
    pub suite: String,
    /// Strict unreviewed proof-label JSONL.
    pub labels: String,
}

/// Fixed refusals with no private text or nested gateway error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftError {
    /// Private persistence failed or changed receipts were detected.
    Journal,
    /// An earlier reserved attempt ended without a terminal receipt.
    Interrupted,
    /// Invalid model output or invented review.
    Candidate,
    /// Source identity, window, or anchor mismatch.
    Source,
    /// Previously accepted independent family.
    DuplicateFamily,
    /// Card is absent, changed, or not an answerer.
    Card,
    /// Prompt bytes do not match their frozen digest.
    Prompt,
    /// Invalid or exceeded work, byte, or token limit.
    Budget,
    /// The adapter cannot render the exact chat template.
    Unsupported,
    /// Model refused, including insufficient free room.
    Gateway,
    /// The per-window deadline expired.
    Timeout,
}

impl fmt::Display for DraftError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Journal => "draft_journal",
            Self::Interrupted => "draft_interrupted",
            Self::Candidate => "draft_candidate",
            Self::Source => "draft_source",
            Self::DuplicateFamily => "draft_duplicate_family",
            Self::Card => "draft_card",
            Self::Prompt => "draft_prompt",
            Self::Budget => "draft_budget",
            Self::Gateway => "draft_gateway",
            Self::Unsupported => "draft_render_unsupported",
            Self::Timeout => "draft_timeout",
        })
    }
}
impl Error for DraftError {}

/// Closed model envelope; nested JSON strings preserve duplicate-key checks.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateWire {
    /// One question in the existing suite format.
    suite: String,
    /// One unreviewed label in G06's existing format.
    labels: String,
}

/// Revalidate model anchors against exactly the approved source window.
///
/// # Errors
/// Returns fixed codes for malformed candidates, changed sources or duplicate families.
pub fn check_candidate(
    output: &str,
    window: &DraftWindow,
    families: &BTreeSet<String>,
) -> Result<DraftCandidate, DraftError> {
    window_text(window)?;
    let wire: CandidateWire = serde_json::from_str(output).map_err(|_| DraftError::Candidate)?;
    let suite: Suite = wire.suite.parse().map_err(|_| DraftError::Candidate)?;
    let label: Label = serde_json::from_str(&wire.labels).map_err(|_| DraftError::Candidate)?;
    if suite.questions.len() != 1 || label.id != window.id || label.review.is_some() {
        return Err(DraftError::Candidate);
    }
    let question = suite.questions.first().ok_or(DraftError::Candidate)?;
    if question.question.trim().is_empty()
        || question
            .expected
            .iter()
            .any(|section| section.source_ref != window.source_ref)
    {
        return Err(DraftError::Candidate);
    }
    for anchor in label
        .proofs
        .iter()
        .flat_map(|proof| &proof.links)
        .flat_map(|link| &link.anchors)
    {
        if anchor.span[0] < window.span[0] || anchor.span[1] > window.span[1] {
            return Err(DraftError::Source);
        }
    }
    let checked = check_labels(
        &suite,
        &wire.labels,
        &Digest::of(wire.labels.as_bytes()),
        Stage::Draft,
        |reference, digest| {
            Ok::<_, Infallible>(
                (reference == window.source_ref && *digest == window.original).then(|| Original {
                    revision_id: window.revision_id.clone(),
                    bytes: window.source.as_bytes().to_vec(),
                }),
            )
        },
    )
    .map_err(|_| DraftError::Candidate)?;
    let item = checked.items.first().ok_or(DraftError::Candidate)?;
    if families.contains(&item.family) {
        return Err(DraftError::DuplicateFamily);
    }
    Ok(DraftCandidate {
        id: item.id.clone(),
        family: item.family.clone(),
        suite: wire.suite,
        labels: wire.labels,
    })
}

/// Checked UTF-8 window, refusing changed or unsafe inventory data.
fn window_text(window: &DraftWindow) -> Result<&str, DraftError> {
    if !safe_id(&window.id)
        || Digest::of(window.source.as_bytes()) != window.original
        || window.span[0] >= window.span[1]
    {
        return Err(DraftError::Source);
    }
    window
        .source
        .get(window.span[0]..window.span[1])
        .ok_or(DraftError::Source)
}

/// Frozen per-window generation limits; cumulative reservation belongs to the durable caller.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftBudget {
    /// Maximum UTF-8 bytes of prompt plus source payload.
    pub input_bytes: usize,
    /// Maximum input tokens, as counted by the pinned model.
    pub input_tokens: u32,
    /// Maximum generated tokens, no more than the gateway's 1,024.
    pub output_tokens: u32,
    /// Maximum generated UTF-8 bytes before decoding.
    pub output_bytes: usize,
    /// Total deadline covering tokenization and inference for one window.
    pub deadline_ms: u64,
}

/// Frozen inputs to one drafting call through the replaceable existing model port.
pub struct DraftRequest<'a> {
    /// Loaded local answerer card.
    pub card: &'a ModelCard,
    /// Independently pinned expected card digest.
    pub card_digest: &'a Digest,
    /// Private drafting instructions.
    pub prompt: &'a str,
    /// Digest frozen before reading acceptance sources.
    pub prompt_digest: &'a Digest,
    /// Approved input window.
    pub window: &'a DraftWindow,
    /// Per-window limits.
    pub budget: DraftBudget,
}

/// Generate one candidate using free room only, bounded by bytes, exact tokens and time.
/// The caller must admit a local router and persist its reservation before this call.
///
/// # Errors
/// Returns fixed codes only; neither source text nor raw gateway errors escape.
pub async fn draft_window<P: ModelPort + Sync>(
    port: &P,
    request: &DraftRequest<'_>,
    families: &BTreeSet<String>,
) -> Result<DraftCandidate, DraftError> {
    let chat = checked_chat(request)?;
    timeout(Duration::from_millis(request.budget.deadline_ms), async {
        let rendered = port
            .render_chat(request.card, Room::Free, &chat)
            .await
            .map_err(|error| match error {
                GatewayError::Unsupported => DraftError::Unsupported,
                _ => DraftError::Gateway,
            })?;
        let input_tokens = port
            .tokenize(request.card, Room::Free, &rendered)
            .await
            .map_err(|_| DraftError::Gateway)?
            .len();
        if input_tokens > request.budget.input_tokens as usize {
            return Err(DraftError::Budget);
        }
        let output = port
            .chat(request.card, Room::Free, &chat)
            .await
            .map_err(|_| DraftError::Gateway)?;
        if output.len() > request.budget.output_bytes {
            return Err(DraftError::Budget);
        }
        let tokens = port
            .tokenize(request.card, Room::Free, &output)
            .await
            .map_err(|_| DraftError::Gateway)?;
        if tokens.len() > request.budget.output_tokens as usize {
            return Err(DraftError::Budget);
        }
        check_candidate(&output, request.window, families)
    })
    .await
    .map_err(|_| DraftError::Timeout)?
}

/// Construct the bound prompt without allowing source text to replace instructions.
pub(super) fn checked_chat(request: &DraftRequest<'_>) -> Result<ChatRequest, DraftError> {
    let card = request.card;
    if card.digest() != request.card_digest || card.fields().role != Role::Answerer {
        return Err(DraftError::Card);
    }
    if request.prompt.trim().is_empty()
        || Digest::of(request.prompt.as_bytes()) != *request.prompt_digest
    {
        return Err(DraftError::Prompt);
    }
    let budget = request.budget;
    if budget.input_bytes == 0
        || budget.output_bytes == 0
        || budget.deadline_ms == 0
        || budget.input_tokens == 0
        || !(1..=1024).contains(&budget.output_tokens)
        || budget
            .input_tokens
            .checked_add(budget.output_tokens)
            .is_none_or(|total| total > card.fields().limits.context_tokens.get())
        || card
            .fields()
            .limits
            .output_tokens
            .is_none_or(|limit| budget.output_tokens > limit.get())
    {
        return Err(DraftError::Budget);
    }
    let window = request.window;
    let payload = serde_json::json!({"id": window.id, "source_ref":window.source_ref,
        "original":window.original, "span":window.span, "source":window_text(window)?})
    .to_string();
    if request
        .prompt
        .len()
        .checked_add(payload.len())
        .is_none_or(|bytes| bytes > budget.input_bytes)
    {
        return Err(DraftError::Budget);
    }
    let mut chat = ChatRequest::new(
        vec![
            Message {
                speaker: Speaker::System,
                content: request.prompt.to_owned(),
            },
            Message {
                speaker: Speaker::User,
                content: payload,
            },
        ],
        budget.output_tokens,
    );
    if let Some(identity) = card.identity()
        && let Capability::Supported(controls) = &identity.invocation.reasoning
    {
        chat.chat_template_kwargs.clone_from(controls);
    }
    Ok(chat)
}

impl fmt::Debug for DraftWindow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DraftWindow")
            .finish_non_exhaustive()
    }
}
impl fmt::Debug for DraftRequest<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DraftRequest")
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for DraftCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DraftCandidate")
            .finish_non_exhaustive()
    }
}
