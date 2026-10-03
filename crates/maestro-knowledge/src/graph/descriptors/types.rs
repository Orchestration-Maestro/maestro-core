//! Canonical disposable descriptor values; concatenated text is never a quote.
//! Serialized fields stay lexicographically ordered for the frozen /1 preimages.

use super::qualifiers::DescriptorQualifiers;
use maestro_kernel::{artifact::Digest, evidence::Span};
use serde::{Deserialize, Serialize};
use std::{error, fmt};

/// Immutable text composition identity.
pub const BUILDER_VERSION: &str = "maestro-source-descriptors/1";

/// One exact collection/generation/version view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DescriptorPin {
    /// Source collection.
    pub collection_id: String,
    /// Immutable generation membership.
    pub generation_id: i64,
    /// Exact source version, or an unrestricted version view.
    pub version: Option<String>,
}

/// Separate original pointer for one verified span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePointer {
    /// Canonical containing block.
    pub block_id: String,
    /// Digest of the original bytes, not the concatenated descriptor.
    pub quote_digest: Digest,
    /// Original revision.
    pub revision_id: String,
    /// Half-open original UTF-8 byte span.
    pub span: Span,
}

/// A deterministic retrieval document, never authoritative evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Descriptor {
    /// Frozen attached claim set.
    pub claim_set: Digest,
    /// Eligible source/review state at build time; current authority must recheck.
    pub eligible: bool,
    /// Content-addressed application ID.
    pub id: Digest,
    /// `claim` or `entity`; claims are queried first by the linker.
    pub kind: String,
    /// Exact source view.
    pub pin: DescriptorPin,
    /// Disjoint original spans, kept separate even when text is concatenated.
    pub pointers: Vec<SourcePointer>,
    /// Source qualifiers preserved without inferred ordering.
    pub qualifiers: DescriptorQualifiers,
    /// Frozen identity resolution.
    pub resolution: Digest,
    /// Entity or claim application ID to recheck before traversal.
    pub target: Digest,
    /// Canonical index text composed only from names, kinds and source bytes.
    pub text: String,
}

impl Descriptor {
    /// Immutable canonical content identity, excluding its own ID.
    #[expect(
        clippy::expect_used,
        reason = "Typed descriptor fields contain only JSON-safe strings, integers and booleans."
    )]
    pub(super) fn identity(&self) -> Digest {
        let body = (
            BUILDER_VERSION,
            &self.target,
            &self.kind,
            &self.text,
            &self.pointers,
            &self.pin,
            &self.claim_set,
            &self.resolution,
            &self.qualifiers,
            self.eligible,
        );
        Digest::of(&serde_json::to_vec(&body).expect("typed descriptor identity serializes"))
    }
}

/// Compatibility identity for reusable embeddings and projection readiness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DescriptorProfile {
    /// Digest of the immutable builder version.
    pub builder: Digest,
    /// Dense layout dimensions pinned by the embedding card.
    pub dimensions: usize,
    /// Immutable embedding card digest.
    pub embedding: Digest,
    /// Development-selected linking profile digest.
    pub linking: Digest,
    /// Existing dense preprocessing profile digest.
    pub preprocessing: Digest,
}

/// Optional readiness, stored only alongside the disposable vector projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DescriptorReceipt {
    /// Ordered canonical descriptor content digest.
    pub content: Digest,
    /// Number of verified descriptor points.
    pub count: usize,
    /// Exact projected view.
    pub pin: DescriptorPin,
    /// Complete compatibility identity.
    pub profile: DescriptorProfile,
}

/// Sanitized refusal at the descriptor boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorError {
    /// Sanitized boundary message, never original source text.
    Refused(String),
    /// An unresolved entity holds the entire build for sourced review.
    HeldForReview(Digest),
}

impl fmt::Display for DescriptorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(message) => formatter.write_str(message),
            Self::HeldForReview(entity) => {
                write!(formatter, "entity {} held for review", entity.as_str())
            }
        }
    }
}

impl error::Error for DescriptorError {}

/// Ordered descriptor content in the frozen /1 field order.
#[expect(
    clippy::expect_used,
    reason = "Typed descriptors contain only JSON-safe strings, integers and booleans."
)]
pub(super) fn content_digest(documents: &[Descriptor]) -> Digest {
    Digest::of(&serde_json::to_vec(documents).expect("typed descriptor content serializes"))
}
