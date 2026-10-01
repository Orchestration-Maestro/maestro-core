//! Immutable baseline plus scoped local overlays; pointer replacement commits.
use super::{
    manifest::{
        Activation, ActivationAuthority, Commit, ConfigurationWriter, Notify, Proposal, WriteError,
    },
    storage::{self, Recovery},
};
use crate::{
    Principal, Ref, Refusal, ResourceSource, policy::manifest::AcquisitionManifest, validate,
};
use maestro_kernel::{
    acquisition::{Handle, Progress, Reason, Receipts, Status},
    artifact::Digest,
    filesystem,
};
use maestro_knowledge::collection::Declaration;
use std::{
    fmt,
    path::{Path, PathBuf},
};

/// Caller-authenticated ports; no catalog imports, hard-coded store or gate policy.
pub struct WriterContext<'a> {
    /// Immutable baseline read adapter; no write operation exists on this port.
    pub source: &'a dyn ResourceSource,
    /// Exact collection link, never rebound by an overlay.
    pub collection: &'a Declaration,
    /// Current external principal/platform context.
    pub principal: &'a Principal<'a>,
    /// Protected immutable reports and evidence, with current access checks.
    pub receipts: &'a dyn Receipts,
    /// N32–N34's current gate policy; disabled implementations hold.
    pub authority: &'a dyn ActivationAuthority,
    /// Only post-commit content-free progress is delivered.
    pub notify: &'a dyn Notify,
    /// Atomic pointer adapter, separate from baseline resolution.
    pub commit: &'a dyn Commit,
}

/// Standard local overlay writer, shared by direct-file and catalog baselines.
/// The root must already be caller-bound and protected. Baseline bytes are never
/// copied into or edited in the overlay; only its manifest pointer is writable.
pub struct LocalWriter<'a> {
    /// Caller-bound directory, never a policy-provided path.
    root: PathBuf,
    /// Frozen manifest binding; opening existing state cannot silently rebind it.
    initial: AcquisitionManifest,
    /// Replaceable read, storage, gate and delivery adapters.
    context: WriterContext<'a>,
}
impl fmt::Debug for LocalWriter<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalWriter")
            .finish_non_exhaustive()
    }
}
impl fmt::Debug for WriterContext<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WriterContext")
            .finish_non_exhaustive()
    }
}

impl<'a> LocalWriter<'a> {
    /// Open/recover an existing overlay, or initialize an empty local manifest.
    /// Initialization requires baseline-as-active and no proposed/active overlays.
    /// # Errors
    /// Unsafe root, untrusted baseline, silent rebind, corrupt state or held recovery.
    pub fn open(
        root: &Path,
        context: WriterContext<'a>,
        initial: &AcquisitionManifest,
    ) -> Result<Self, WriteError> {
        let writer = Self {
            root: root.into(),
            initial: initial.clone(),
            context,
        };
        let _lock = storage::lock(root)?;
        if storage::read(root, "manifest.json")?.is_none() {
            if initial.active != initial.baseline
                || !initial.proposals.is_empty()
                || !initial.activations.is_empty()
            {
                return Err(Refusal::Invalid.into());
            }
            let mut manifest = initial.clone();
            manifest.effective_digest = writer.digest(&manifest)?;
            let bytes = storage::encode(&manifest)?;
            let _checked: AcquisitionManifest = storage::decode(&bytes)?;
            filesystem::atomic_replace(&root.join("manifest.json"), &bytes)?;
        }
        writer.recover()?;
        writer.load()?;
        Ok(writer)
    }

    /// Read complete current effective authority with fresh baseline/access/gates.
    /// In-flight callers keep the returned immutable processing identities pinned.
    /// # Errors
    /// Baseline revocation/substitution, corrupt overlays or currently held gates.
    pub fn current(&self) -> Result<AcquisitionManifest, WriteError> {
        let _lock = storage::lock(&self.root)?;
        self.recover()?;
        let manifest = self.load()?;
        if manifest.active != manifest.baseline {
            self.eligible(&manifest.active)?;
        }
        Ok(manifest)
    }

    /// Revalidate the full N03 closure and exact inherited manifest metadata.
    fn digest(&self, manifest: &AcquisitionManifest) -> Result<Digest, WriteError> {
        let checked = validate(
            self.context.source,
            self.context.collection,
            self.context.principal,
        )?;
        let metadata = &checked.policy().resource;
        if manifest.baseline != *checked.reference()
            || manifest.baseline != self.initial.baseline
            || manifest.baseline_kind != self.initial.baseline_kind
        {
            return Err(WriteError::Conflict);
        }
        if manifest.resource != self.initial.resource
            || manifest.resource.collection_id != metadata.collection_id
            || manifest.resource.visibility != metadata.visibility
            || manifest.resource.scope_tags != metadata.scope_tags
            || manifest.resource.owner_ref != metadata.owner_ref
        {
            return Err(Refusal::Access.into());
        }
        // Canonical baseline content plus ordered immutable activation identities.
        Ok(Digest::of(&storage::encode(&(
            checked.policy(),
            &manifest.activations,
        ))?))
    }

    /// Reread and verify pointer state while the collection lock is held.
    fn load(&self) -> Result<AcquisitionManifest, WriteError> {
        let bytes = storage::read(&self.root, "manifest.json")?.ok_or(WriteError::Storage)?;
        let manifest: AcquisitionManifest = storage::decode(&bytes)?;
        if manifest.effective_digest != self.digest(&manifest)? {
            return Err(Refusal::Digest.into());
        }
        let expected = manifest.activations.last().unwrap_or(&manifest.baseline);
        if &manifest.active != expected {
            return Err(Refusal::Invalid.into());
        }
        for reference in &manifest.proposals {
            let _proposal: Proposal =
                storage::artifact(self.context.receipts, self.context.principal, reference)?;
        }
        for reference in &manifest.activations {
            let _activation: Activation =
                storage::artifact(self.context.receipts, self.context.principal, reference)?;
        }
        Ok(manifest)
    }

    /// Fresh expected baseline and previous-active CAS, never last-writer-wins.
    fn expected(&self, baseline: &Ref, active: &Ref) -> Result<AcquisitionManifest, WriteError> {
        let manifest = self.load()?;
        if &manifest.baseline != baseline || &manifest.active != active {
            return Err(WriteError::Conflict);
        }
        Ok(manifest)
    }

    /// Current proposal/gate access and policy eligibility for an activation.
    fn eligible(&self, reference: &Ref) -> Result<Proposal, WriteError> {
        let activation: Activation =
            storage::artifact(self.context.receipts, self.context.principal, reference)?;
        let proposal =
            storage::proposal(self.context.receipts, self.context.principal, &activation)?;
        storage::inputs(
            self.context.receipts,
            self.context.principal,
            &proposal,
            activation.gate,
        )?;
        self.context
            .authority
            .check(&proposal, activation.gate, self.context.principal)?;
        Ok(proposal)
    }

    /// Commit a complete pointer after durable scoped artifacts and recovery marker.
    fn commit(
        &self,
        mut manifest: AcquisitionManifest,
        event: Option<Progress>,
    ) -> Result<(), WriteError> {
        manifest.effective_digest = self.digest(&manifest)?;
        let old = storage::read(&self.root, "manifest.json")?.ok_or(WriteError::Storage)?;
        let new = storage::encode(&manifest)?;
        let _checked: AcquisitionManifest = storage::decode(&new)?;
        let recovery = Recovery {
            old: Digest::of(&old),
            new: Digest::of(&new),
            event,
        };
        filesystem::atomic_replace(
            &self.root.join("recovery.json"),
            &storage::encode(&recovery)?,
        )?;
        self.context.commit.replace(&self.root, &new)?;
        self.recover()
    }

    /// Complete only post-commit delivery, or discard an uncommitted marker.
    fn recover(&self) -> Result<(), WriteError> {
        let Some(bytes) = storage::read(&self.root, "recovery.json")? else {
            return Ok(());
        };
        let marker: Recovery = storage::decode(&bytes)?;
        let pointer = storage::read(&self.root, "manifest.json")?.ok_or(WriteError::Storage)?;
        let digest = Digest::of(&pointer);
        if digest == marker.old {
            return storage::clear(&self.root);
        }
        if digest != marker.new {
            return Err(WriteError::Conflict);
        }
        let manifest = self.load()?;
        if let Some(event) = marker.event {
            if event.receipt.to_string() != manifest.active.id
                || event.status != Status::Complete
                || event.reason != Reason::None
            {
                return Err(Refusal::Invalid.into());
            }
            self.eligible(&manifest.active)?;
            self.context.notify.notify(event)?;
        }
        storage::clear(&self.root)
    }

    /// Store the entire proposal under all inherited scope tags before exposure.
    fn retain_proposal(&self, proposal: &Proposal) -> Result<Ref, WriteError> {
        let mut handles = proposal.evidence.clone();
        handles.push(proposal.report);
        storage::accessible(self.context.receipts, self.context.principal, &handles)?;
        let bytes = storage::encode(proposal)?;
        // Decode even Rust-constructed DTOs to enforce strict Ref/string shapes.
        let _proposal: Proposal = storage::decode(&bytes)?;
        storage::retain(
            self.context.receipts,
            &self.initial.resource.scope_tags,
            &bytes,
            &handles,
        )
    }

    /// Store and expose an activation after full input and current gate checks.
    fn apply(
        &self,
        mut manifest: AcquisitionManifest,
        reference: &Ref,
        gate: Handle,
        restores: Option<Ref>,
    ) -> Result<Ref, WriteError> {
        let proposal: Proposal =
            storage::artifact(self.context.receipts, self.context.principal, reference)?;
        if proposal.expected_baseline != manifest.baseline
            || proposal.expected_active != manifest.active
        {
            return Err(WriteError::Conflict);
        }
        storage::inputs(
            self.context.receipts,
            self.context.principal,
            &proposal,
            gate,
        )?;
        self.context
            .authority
            .check(&proposal, gate, self.context.principal)?;
        let activation = Activation {
            proposal: reference.clone(),
            gate,
            previous: manifest.active.clone(),
            restores,
        };
        let proposal_handle = reference.id.parse().map_err(|_| Refusal::Invalid)?;
        let identity = storage::retain(
            self.context.receipts,
            &manifest.resource.scope_tags,
            &storage::encode(&activation)?,
            &[proposal_handle, gate],
        )?;
        manifest.active = identity.clone();
        manifest.activations.push(identity.clone());
        let event = Progress {
            receipt: identity.id.parse().map_err(|_| Refusal::Invalid)?,
            status: Status::Complete,
            reason: Reason::None,
        };
        self.commit(manifest, Some(event))?;
        Ok(identity)
    }
}
impl ConfigurationWriter for LocalWriter<'_> {
    fn propose(
        &self,
        expected_baseline: &Ref,
        expected_active: &Ref,
        proposal: &Proposal,
    ) -> Result<Ref, WriteError> {
        let _lock = storage::lock(&self.root)?;
        self.recover()?;
        let mut manifest = self.expected(expected_baseline, expected_active)?;
        if &proposal.expected_baseline != expected_baseline
            || &proposal.expected_active != expected_active
            || proposal.rollback != *expected_active
        {
            return Err(WriteError::Conflict);
        }
        if proposal.changes.is_empty() || proposal.evidence.is_empty() {
            return Err(Refusal::Invalid.into());
        }
        let reference = self.retain_proposal(proposal)?;
        manifest.proposals.push(reference.clone());
        self.commit(manifest, None)?;
        Ok(reference)
    }
    fn activate(
        &self,
        expected_baseline: &Ref,
        expected_active: &Ref,
        candidate: &Ref,
        gate_receipt: Handle,
    ) -> Result<Ref, WriteError> {
        let _lock = storage::lock(&self.root)?;
        self.recover()?;
        let manifest = self.expected(expected_baseline, expected_active)?;
        if !manifest.proposals.contains(candidate) {
            return Err(Refusal::Missing.into());
        }
        self.apply(manifest, candidate, gate_receipt, None)
    }
    fn rollback(
        &self,
        expected_active: &Ref,
        previous: &Ref,
        current_authority: Handle,
    ) -> Result<Ref, WriteError> {
        let _lock = storage::lock(&self.root)?;
        self.recover()?;
        let mut manifest = self.load()?;
        if &manifest.active != expected_active {
            return Err(WriteError::Conflict);
        }
        let current: Activation = storage::artifact(
            self.context.receipts,
            self.context.principal,
            expected_active,
        )?;
        if &current.previous != previous {
            return Err(WriteError::Held);
        }
        let original = storage::proposal(self.context.receipts, self.context.principal, &current)?;
        let restored = if previous == &manifest.baseline {
            None
        } else {
            Some(self.eligible(previous)?)
        };
        let proposal = Proposal {
            expected_baseline: manifest.baseline.clone(),
            expected_active: manifest.active.clone(),
            changes: restored
                .as_ref()
                .map_or_else(Vec::new, |value| value.changes.clone()),
            candidate: restored.as_ref().map_or_else(
                || manifest.baseline.digest.clone(),
                |value| value.candidate.clone(),
            ),
            evidence: vec![current_authority],
            report: original.report,
            rollback: manifest.active.clone(),
        };
        let reference = self.retain_proposal(&proposal)?;
        manifest.proposals.push(reference.clone());
        self.apply(
            manifest,
            &reference,
            current_authority,
            Some(previous.clone()),
        )
    }
}
