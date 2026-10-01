//! Typed local overlays and replaceable activation/recovery policy seams.
use crate::{Principal, Ref, Refusal, policy::shape};
use maestro_kernel::{
    acquisition::{Handle, Progress},
    artifact::Digest,
    filesystem,
    scope::ScopeSet,
    store::Database,
};
use maestro_knowledge::strict_json::{nullable_object, object};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, io, path::Path};

/// Closed automatic edits. Definitions stay immutable referenced resources;
/// N32 checks protected fields even when a profile is indirectly substituted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    /// Select an approved immutable extraction profile for a declared source.
    SelectProfile {
        /// Declared source identity.
        #[serde(deserialize_with = "shape::id")]
        source_id: String,
        /// Exact immutable profile.
        #[serde(deserialize_with = "object")]
        profile: Ref,
        /// Pinned source selection receipt and its content evidence.
        #[serde(deserialize_with = "object")]
        selection: Ref,
    },
    /// Replace cleanup selection, not its protected definition.
    SetCleanup {
        /// Immutable selected rules.
        #[serde(deserialize_with = "object")]
        rules: Ref,
    },
    /// Select a qualified S1 strategy within the model's limit.
    SetS1ChunkStrategy {
        /// Immutable strategy selection.
        #[serde(deserialize_with = "object")]
        strategy: Ref,
    },
    /// Select immutable deduplication keys.
    SetDedupKeys {
        /// Exact key selection.
        #[serde(deserialize_with = "object")]
        keys: Ref,
    },
    /// Add, never edit/remove, a knowledge exclusion.
    AddKnowledgeExclusion {
        /// New immutable exclusion entry.
        #[serde(deserialize_with = "object")]
        entry: Ref,
    },
    /// Add a new asset-only entry.
    AddAssetOnly {
        /// New immutable disposition entry.
        #[serde(deserialize_with = "object")]
        entry: Ref,
    },
}

/// Immutable proposal payload. Full observations, uncertainty, pinned sample,
/// cohort/profile identities, rule diff and outcomes live in its scoped report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    /// Immutable baseline the proposal was evaluated against.
    #[serde(deserialize_with = "object")]
    pub expected_baseline: Ref,
    /// Previous active identity, distinct from a proposed version.
    #[serde(deserialize_with = "object")]
    pub expected_active: Ref,
    /// Only the closed edit set can be decoded.
    pub changes: Vec<Change>,
    /// Digest bound by the activation authority's complete gate report.
    pub candidate: Digest,
    /// Opaque evidence handles inherited transitively by the stored proposal.
    pub evidence: Vec<Handle>,
    /// Immutable scoped report, retained before manifest exposure.
    pub report: Handle,
    /// Previous still-authorized processing configuration.
    #[serde(deserialize_with = "object")]
    pub rollback: Ref,
}

/// Stored activation identity includes gate, report and rollback bindings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Activation {
    /// Exact immutable proposal handle/digest.
    #[serde(deserialize_with = "object")]
    pub proposal: Ref,
    /// Scoped gate receipt, not a self-asserted score.
    pub gate: Handle,
    /// Previous active configuration for rollback.
    #[serde(deserialize_with = "object")]
    pub previous: Ref,
    /// Explicit earlier processing version restored by a rollback, not a rule replay.
    #[serde(deserialize_with = "nullable_object")]
    pub restores: Option<Ref>,
}

/// Content-free local write errors; private report bytes never enter diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteError {
    /// Exact expected state changed.
    Conflict,
    /// Missing/inconclusive/currently revoked activation authority.
    Held,
    /// Shared resource or current-access validator refused input.
    Refused(Refusal),
    /// Local durable storage or scoped receipt storage failed.
    Storage,
}
impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "configuration write refused: {self:?}")
    }
}
impl Error for WriteError {}
impl From<Refusal> for WriteError {
    fn from(error: Refusal) -> Self {
        Self::Refused(error)
    }
}
impl From<io::Error> for WriteError {
    fn from(_: io::Error) -> Self {
        Self::Storage
    }
}

/// Fresh kernel grants, acquired for each writer operation, never cached at open.
pub trait CurrentGrants: fmt::Debug {
    /// Read the authenticated principal's current read scopes.
    /// # Errors
    /// An unavailable grant store refuses the operation.
    fn visible(&self, principal: &str) -> Result<ScopeSet, WriteError>;
}
impl CurrentGrants for Database {
    fn visible(&self, principal: &str) -> Result<ScopeSet, WriteError> {
        Database::visible(self, principal).map_err(|_| WriteError::Storage)
    }
}

/// N32–N34 supply protected-field, matrix, ledger and safe-boundary gates.
/// Every commit, current active read and committed recovery asks this port again;
/// adapters must validate exact candidate/baseline/cohort/suite/profile bindings.
pub trait ActivationAuthority: fmt::Debug {
    /// Revalidate current activation or rollback eligibility, failing closed.
    /// # Errors
    /// Missing gates or revoked authority returns Held, never implicit approval.
    fn check(
        &self,
        proposal: &Proposal,
        gate: Handle,
        principal: &Principal<'_>,
    ) -> Result<(), WriteError>;
}
/// Disabled/default authority: no activation is admissible.
#[derive(Debug, Default)]
pub struct HeldAuthority;
impl ActivationAuthority for HeldAuthority {
    fn check(&self, _: &Proposal, _: Handle, _: &Principal<'_>) -> Result<(), WriteError> {
        Err(WriteError::Held)
    }
}

/// Post-commit journal/notification adapter. Deduplicate by the opaque handle:
/// recovery may retry a delivery after a crash between delivery and marker removal.
pub trait Notify: fmt::Debug {
    /// Emit only N06's fixed status and access-checked opaque report handle.
    /// # Errors
    /// Failure retains the recovery marker for another delivery attempt.
    fn notify(&self, event: Progress) -> Result<(), WriteError>;
}

/// Replaceable atomic manifest pointer adapter; the pointer is the commit point.
pub trait Commit: fmt::Debug {
    /// Replace complete bytes in the caller-bound protected overlay only.
    /// # Errors
    /// Old or new complete bytes survive; the durable recovery marker disambiguates.
    fn replace(&self, root: &Path, bytes: &[u8]) -> Result<(), WriteError>;
}
/// Standard durable replacement, with the kernel's platform semantics.
#[derive(Debug)]
pub struct AtomicCommit;
impl Commit for AtomicCommit {
    fn replace(&self, root: &Path, bytes: &[u8]) -> Result<(), WriteError> {
        filesystem::atomic_replace(&root.join("manifest.json"), bytes).map_err(Into::into)
    }
}

/// One contract for direct-file and later installed catalog immutable baselines.
pub trait ConfigurationWriter: fmt::Debug {
    /// Store a proposal/report before recording its identity, without activating it.
    /// # Errors
    /// Stale identities, invalid shape, untrusted baseline or inaccessible evidence.
    fn propose(
        &self,
        expected_baseline: &Ref,
        expected_active: &Ref,
        proposal: &Proposal,
    ) -> Result<Ref, WriteError>;
    /// Atomically expose an eligible proposal after fresh CAS and authority checks.
    /// # Errors
    /// Conflicts, missing evidence, held eligibility or durable storage failure.
    fn activate(
        &self,
        expected_baseline: &Ref,
        expected_active: &Ref,
        candidate: &Ref,
        gate_receipt: Handle,
    ) -> Result<Ref, WriteError>;
    /// Restore an earlier version only with fresh authority via the same gate seam.
    /// # Errors
    /// Stale current state or revoked/missing previous authority holds rollback.
    fn rollback(
        &self,
        expected_active: &Ref,
        previous: &Ref,
        current_authority: Handle,
    ) -> Result<Ref, WriteError>;
}
