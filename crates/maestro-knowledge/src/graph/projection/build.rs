//! Backend-neutral authoritative build inputs and successful publication result.
use super::{port::ProjectionScope, settings::EngineSettings};
use maestro_kernel::{artifact::Digest, facts::ProjectionReceipt, job::Lease};

/// Exact authoritative build identities and the matching kernel project lease.
#[derive(Debug, Clone)]
pub struct ProjectionBuild {
    /// Pinned collection and generation.
    pub scope: ProjectionScope,
    /// Attached authoritative claim set.
    pub claim_set_id: Digest,
    /// Caller-held `knowledge.graph.project` lease, never inferred from a PID.
    pub lease: Lease,
}

/// Result of durable installation and successful kernel readiness recording.
/// The frozen lock is carried here without changing the persisted receipt format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedProjection {
    /// Exact immutable kernel receipt.
    pub receipt: ProjectionReceipt,
    /// Exact settings and frozen non-resource lock admitted by the factory.
    pub settings: EngineSettings,
}
