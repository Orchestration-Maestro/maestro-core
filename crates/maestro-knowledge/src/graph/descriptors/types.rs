//! Canonical disposable descriptor values; concatenated text is never a quote.

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
    /// Original revision.
    pub revision_id: String,
    /// Canonical containing block.
    pub block_id: String,
    /// Half-open original UTF-8 byte span.
    pub span: Span,
    /// Digest of the original bytes, not the concatenated descriptor.
    pub quote_digest: Digest,
}

/// A deterministic retrieval document, never authoritative evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Descriptor {
    /// Content-addressed application ID.
    pub id: Digest,
    /// Entity or claim application ID to recheck before traversal.
    pub target: Digest,
    /// `claim` or `entity`; claims are queried first by the linker.
    pub kind: String,
    /// Canonical index text composed only from names, kinds and source bytes.
    pub text: String,
    /// Disjoint original spans, kept separate even when text is concatenated.
    pub pointers: Vec<SourcePointer>,
    /// Exact source view.
    pub pin: DescriptorPin,
    /// Frozen attached claim set.
    pub claim_set: Digest,
    /// Frozen identity resolution.
    pub resolution: Digest,
    /// Source qualifiers preserved without inferred ordering.
    pub qualifiers: serde_json::Value,
    /// Eligible source/review state at build time; current authority must recheck.
    pub eligible: bool,
}

impl Descriptor {
    /// Immutable canonical content identity, excluding its own ID.
    pub(super) fn identity(&self) -> Digest {
        let body = serde_json::json!([
            BUILDER_VERSION,
            self.target,
            self.kind,
            self.text,
            self.pointers,
            self.pin,
            self.claim_set,
            self.resolution,
            self.qualifiers,
            self.eligible
        ]);
        Digest::of(body.to_string().as_bytes())
    }
}

/// Compatibility identity for reusable embeddings and projection readiness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DescriptorProfile {
    /// Digest of the immutable builder version.
    pub builder: Digest,
    /// Immutable embedding card digest.
    pub embedding: Digest,
    /// Existing dense preprocessing profile digest.
    pub preprocessing: Digest,
    /// Development-selected linking profile digest.
    pub linking: Digest,
    /// Dense layout dimensions pinned by the embedding card.
    pub dimensions: usize,
}

/// Optional readiness, stored only alongside the disposable vector projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DescriptorReceipt {
    /// Exact projected view.
    pub pin: DescriptorPin,
    /// Complete compatibility identity.
    pub profile: DescriptorProfile,
    /// Ordered canonical descriptor content digest.
    pub content: Digest,
    /// Number of verified descriptor points.
    pub count: usize,
}

/// Sanitized refusal at the descriptor boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorError(
    /// Sanitized boundary message, never original source text.
    pub String,
);

impl fmt::Display for DescriptorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl error::Error for DescriptorError {}
