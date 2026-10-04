//! Backend-neutral authoritative build inputs and successful publication result.
use super::{port::ProjectionScope, settings::EngineSettings};
use maestro_kernel::{artifact::Digest, facts::ProjectionReceipt, job::Lease};

/// Exact authoritative build identities and the matching kernel project lease.
#[derive(Debug, Clone)]
pub struct ProjectionBuild {
    /// Kernel-reserved immutable native build identity.
    pub build_id: i64,
    /// Pinned collection and generation.
    pub scope: ProjectionScope,
    /// Attached authoritative claim set.
    pub claim_set_id: Digest,
    /// Frozen resolution chosen explicitly by the caller.
    pub resolution_id: Digest,
    /// Exact version of the pinned resolver.
    pub resolver_version: String,
    /// Versioned typed settings identity admitted for this build.
    pub settings_identity: Digest,
    /// Complete frozen non-resource lock admitted for this build.
    pub frozen_lock: Digest,
    /// Caller-held `knowledge.graph.project` lease, never inferred from a PID.
    pub lease: Lease,
}

/// Result of durable installation and successful kernel readiness recording.
/// The returned settings match the durable receipt pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedProjection {
    /// Exact immutable kernel receipt.
    pub receipt: ProjectionReceipt,
    /// Exact settings and frozen non-resource lock admitted by the factory.
    pub settings: EngineSettings,
}
