//! Frozen build inputs and durable receipts.

use super::types::{Claim, ClaimRecord, Provenance};
use crate::artifact::Digest;
use ulid::Ulid;

/// Durable limits for one extraction build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budget {
    /// Maximum accepted claims.
    pub max_claims: usize,
    /// Maximum rejection receipts retained.
    pub max_rejections: usize,
}
/// Frozen inputs of one graph build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildPlan {
    /// Target collection.
    pub collection_id: String,
    /// Extractor and profile identity.
    pub provenance: Provenance,
    /// Ordered revision IDs, one per batch.
    pub sources: Vec<String>,
    /// Persistent limits.
    pub budget: Budget,
}
/// A candidate rejection retained with its reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection {
    /// Source revision.
    pub revision_id: String,
    /// Optional canonical block.
    pub block_id: Option<String>,
    /// Sanitized rejection reason.
    pub reason: String,
}
/// One source batch's accepted claims and failed candidates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch {
    /// Its zero-based source position.
    pub ordinal: usize,
    /// Accepted claims.
    pub claims: Vec<Claim>,
    /// Candidate rejections.
    pub rejections: Vec<Rejection>,
}
/// Receipt of a durably committed batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchReceipt {
    /// Batch position.
    pub ordinal: usize,
    /// Source revision.
    pub revision_id: String,
    /// Lease fence number.
    pub lease_number: u64,
    /// Claims admitted by this batch.
    pub claims: Vec<ClaimRecord>,
    /// Total rejected candidates.
    pub rejected: usize,
    /// Rejections retained under build budget.
    pub kept: usize,
}
/// Build state read from the authority database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRecord {
    /// Frozen plan.
    pub plan: BuildPlan,
    /// Committed ordered batch receipts.
    pub batches: Vec<BatchReceipt>,
    /// Retained rejection records.
    pub rejections: Vec<Rejection>,
    /// Set frozen on completion.
    pub claim_set_id: Option<Digest>,
}
impl BuildRecord {
    /// Number of all rejected candidates, including those beyond the retention budget.
    #[must_use]
    pub fn rejected(&self) -> usize {
        self.batches.iter().map(|batch| batch.rejected).sum()
    }
}
/// Verified disposable projection identity recorded by the kernel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionReceipt {
    /// Collection whose generation is projected.
    pub collection_id: String,
    /// Exact immutable generation pin.
    pub generation_id: i64,
    /// Kernel-authoritative claim set this file represents.
    pub claim_set_id: Digest,
    /// Single owned filename; it is never a caller-supplied path.
    pub file_name: String,
    /// Projection schema checked after close/reopen.
    pub schema_version: String,
    /// Count of entity-to-entity knowledge claims projected as edges.
    pub knowledge_edge_count: usize,
    /// Count of catalog dependency edges verified by the projection backend.
    pub catalog_dependency_edge_count: usize,
    /// Count of literal-valued subject claim facts (never edges).
    pub entity_fact_count: usize,
    /// Digest of verified projection application-ID content.
    pub content_digest: Digest,
}

/// Immutable claim set attached to a generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphAttachment {
    /// Generation pinned to the set.
    pub generation_id: i64,
    /// Build that produced it.
    pub job: Ulid,
    /// Frozen claim-set application ID.
    pub claim_set_id: Digest,
}
