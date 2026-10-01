//! Small optional projection and embedding boundary, independent of graph readiness.

use super::types::{Descriptor, DescriptorError, DescriptorReceipt};
use crate::index::PointHit;
use maestro_kernel::{artifact::Digest, gateway::ModelCard};
use std::time::Duration;

/// Reusable output for arms with identical content/model/preprocessing/linking pins.
#[derive(Debug, Clone)]
pub struct EmbeddedDescriptors {
    /// Optional readiness identity carried in the disposable payloads.
    pub(super) receipt: DescriptorReceipt,
    /// Canonical documents in application-ID order.
    pub(super) documents: Vec<Descriptor>,
    /// Checked vectors, one per document.
    pub(super) vectors: Vec<Vec<f32>>,
}

impl EmbeddedDescriptors {
    /// Optional projection identity; the kernel graph receipt is unchanged.
    #[must_use]
    pub fn receipt(&self) -> &DescriptorReceipt {
        &self.receipt
    }
    /// Canonical documents, without treating index text as quotes.
    #[must_use]
    pub fn descriptors(&self) -> &[Descriptor] {
        &self.documents
    }
}

/// Existing model gateway and frozen development profile, supplied by callers.
#[derive(Debug)]
pub struct DescriptorEmbedder<'a, P> {
    /// Existing replaceable model gateway.
    pub models: &'a P,
    /// Pinned existing multilingual embedder card.
    pub card: &'a ModelCard,
    /// Frozen development linking profile digest.
    pub linking: Digest,
    /// Total embedding-call deadline.
    pub deadline: Duration,
}

/// Scoped lookup request; current authority rechecks remain mandatory downstream.
#[derive(Debug, Clone)]
pub struct DescriptorQuery {
    /// `claim` first, then `entity` if selected by the linker.
    pub kind: String,
    /// Once-formatted question embedding from the existing gateway.
    pub vector: Vec<f32>,
    /// Top-k applied only after the scope/pin/version/eligibility filter.
    pub limit: usize,
}

/// Replaceable disposable descriptor projection, never a claim store.
#[expect(
    async_fn_in_trait,
    reason = "The optional projection has local asynchronous adapters."
)]
pub trait DescriptorProjection {
    /// Create or resume the exact digest-bound collection and verify payload equality.
    async fn rebuild(&self, output: &EmbeddedDescriptors) -> Result<(), DescriptorError>;
    /// Refuse readiness unless all canonical payloads and the layout match.
    async fn verify(&self, output: &EmbeddedDescriptors) -> Result<(), DescriptorError>;
    /// Delete only the collection owned by this immutable receipt.
    async fn delete(&self, receipt: &DescriptorReceipt) -> Result<(), DescriptorError>;
    /// Apply scope/pin/version/eligibility filters inside the backend before top-k.
    async fn lookup(
        &self,
        receipt: &DescriptorReceipt,
        query: DescriptorQuery,
    ) -> Result<Vec<PointHit>, DescriptorError>;
}
