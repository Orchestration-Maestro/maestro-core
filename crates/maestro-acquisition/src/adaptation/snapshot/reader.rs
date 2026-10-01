//! One scoped reader for N32's effective model and its immutable payloads.
use super::super::{
    artifacts::{CleanupRules, DedupKeys, S1ChunkStrategy},
    change::{EffectiveConfiguration, selection_key},
    lineage,
    manifest::{Activation, Proposal, WriteError},
    storage,
};
use crate::{
    CheckedPolicy, Principal, Ref, Refusal, ResourceSource,
    extraction::{
        model::Processing,
        outcome::ProfileSelection,
        registry::{LocalRegistry, checked_resolve},
    },
    policy::{manifest::AcquisitionManifest, resource::Resource, schema::SourcePolicy},
    ports::{AdmissionStatus, ImmutableResource},
    validate,
};
use maestro_kernel::{acquisition::Receipts, artifact::Digest};
use maestro_knowledge::{
    collection::Declaration,
    strict_json::{self, object},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// Closed snapshot envelope version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum SnapshotSchema {
    /// Version one.
    #[serde(rename = "maestro-processing-snapshot/1")]
    V1,
}
/// Envelope only: there is no second processing/effective model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProcessingSnapshot {
    /// Inherited resource identity, first in the typed preimage.
    #[serde(flatten)]
    pub resource: Resource<SnapshotSchema>,
    /// The existing closed-change model, now persisted strictly.
    #[serde(deserialize_with = "object")]
    pub effective: EffectiveConfiguration,
}
/// Verified consumer payloads; selections remain in the existing effective type.
#[derive(Debug)]
pub struct ResolvedSnapshot {
    /// Original typed snapshot with all effective claims revalidated.
    pub snapshot: ProcessingSnapshot,
    /// Whole cleanup sets keyed by receipt identity.
    pub cleanup: BTreeMap<String, CleanupRules>,
    /// Supported S1 strategies keyed by receipt identity.
    pub chunks: BTreeMap<String, S1ChunkStrategy>,
    /// Supported S1 key tuples keyed by receipt identity.
    pub dedup: BTreeMap<String, DedupKeys>,
}
/// Request-local adapters; no active pointer, store or authority is introduced.
pub struct SnapshotReader<'a> {
    /// Existing external immutable resource port.
    pub source: &'a dyn ResourceSource,
    /// Existing scoped storage port.
    pub receipts: &'a dyn Receipts,
    /// Exact collection declaration.
    pub collection: &'a Declaration,
    /// Current authenticated principal and grants.
    pub principal: &'a Principal<'a>,
}
impl fmt::Debug for SnapshotReader<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SnapshotReader")
            .finish_non_exhaustive()
    }
}
impl SnapshotReader<'_> {
    /// Read an exact retained snapshot and revalidate all its current inputs.
    /// # Errors
    /// Missing, inaccessible, substituted, unqualified or inconsistent inputs refuse.
    pub fn read(&self, reference: &Ref) -> Result<ResolvedSnapshot, WriteError> {
        let snapshot = storage::canonical_artifact(self.receipts, self.principal.id, reference)?;
        self.resolve(snapshot)
    }
    /// Store initial/candidate bytes with every receipt scope inherited transitively.
    /// Storage does not approve a candidate or grant matrix authority.
    /// # Errors
    /// Strict shape, closure, current access or retention failure refuses.
    pub fn retain(&self, snapshot: &ProcessingSnapshot) -> Result<Ref, WriteError> {
        let bytes = storage::encode(snapshot)?;
        let checked: ProcessingSnapshot = storage::decode(&bytes)?;
        self.resolve(checked)?;
        let links = self.links(snapshot)?;
        storage::retain(self.receipts, &snapshot.resource.scope_tags, &bytes, &links)
    }
    /// Bind exactly one evidence handle to the entire typed snapshot bytes.
    /// # Errors
    /// Arbitrary digest, missing/duplicate handles or noncanonical typed bytes refuse.
    pub fn candidate(&self, proposal: &Proposal) -> Result<(Ref, ResolvedSnapshot), WriteError> {
        let reference = self.candidate_ref(proposal)?;
        let resolved = self.read(&reference)?;
        if resolved.snapshot.effective.baseline != proposal.expected_baseline {
            return Err(WriteError::Held);
        }
        Ok((reference, resolved))
    }
    /// Find the current processing version along N30's existing restore lineage.
    /// # Errors
    /// Invalid lineage, unreadable target or inconsistent rollback binding refuses.
    pub fn current(
        &self,
        manifest: &AcquisitionManifest,
    ) -> Result<(Ref, ResolvedSnapshot), WriteError> {
        lineage::validate(self.receipts, self.principal.id, manifest, &manifest.active)?;
        let mut cursor = manifest.active.clone();
        let mut rollbacks = Vec::new();
        let reference = loop {
            if cursor == manifest.baseline {
                break manifest.processing_baseline.clone();
            }
            let activation: Activation =
                storage::artifact(self.receipts, self.principal.id, &cursor)?;
            let proposal = storage::proposal(self.receipts, self.principal.id, &activation)?;
            let candidate = self.candidate_ref(&proposal)?;
            if let Some(restores) = activation.restores {
                rollbacks.push(candidate);
                cursor = restores;
            } else {
                break candidate;
            }
        };
        if rollbacks.iter().any(|value| value != &reference) {
            return Err(WriteError::Held);
        }
        let resolved = self.read(&reference)?;
        if resolved.snapshot.effective.baseline != manifest.baseline {
            return Err(WriteError::Held);
        }
        Ok((reference, resolved))
    }
    /// Candidate shape/identity without granting executable qualification.
    pub(super) fn candidate_ref(&self, proposal: &Proposal) -> Result<Ref, WriteError> {
        let mut found = None;
        for handle in &proposal.evidence {
            let artifact = self
                .receipts
                .read(self.principal.id, *handle)
                .map_err(|_| WriteError::Storage)?
                .ok_or(Refusal::Access)?;
            if Digest::of(artifact.bytes()) != proposal.candidate {
                continue;
            }
            if found.is_some() {
                return Err(WriteError::Held);
            }
            found = Some(Ref {
                id: handle.to_string(),
                digest: proposal.candidate.clone(),
            });
        }
        found.ok_or(WriteError::Held)
    }
    /// Recheck the immutable baseline, registry, precedence and approved payloads.
    fn resolve(&self, snapshot: ProcessingSnapshot) -> Result<ResolvedSnapshot, WriteError> {
        let checked = validate(self.source, self.collection, self.principal)?;
        let baseline = checked.policy();
        self.resource(&snapshot.resource, baseline)?;
        let effective = &snapshot.effective;
        if effective.baseline != *checked.reference() {
            return Err(WriteError::Held);
        }
        Self::policy(effective, baseline)?;
        Self::protected(effective, &checked)?;
        let registry = checked_resolve(
            &LocalRegistry::new(self.source),
            &baseline.profiles,
            self.principal,
            &checked,
        )
        .map_err(|_| WriteError::Held)?;
        let profiles: BTreeMap<_, _> = registry
            .registry
            .profiles
            .iter()
            .map(|profile| (profile.definition.id.clone(), profile.clone()))
            .collect();
        if effective.profiles != profiles {
            return Err(WriteError::Held);
        }
        self.selections(effective, baseline, &registry.qualified)?;
        let mut resolved = ResolvedSnapshot {
            snapshot,
            cleanup: BTreeMap::new(),
            chunks: BTreeMap::new(),
            dedup: BTreeMap::new(),
        };
        self.payloads(&mut resolved, baseline)?;
        self.decisions(&resolved.snapshot.effective, baseline)?;
        Ok(resolved)
    }
    /// Exact common ownership and visibility, not just the caller's broad grants.
    pub(super) fn resource<S>(
        &self,
        resource: &Resource<S>,
        policy: &SourcePolicy,
    ) -> Result<(), WriteError> {
        let baseline = &policy.resource;
        if resource.collection_id != baseline.collection_id
            || resource.visibility != baseline.visibility
            || resource.scope_tags != baseline.scope_tags
            || resource.owner_ref != baseline.owner_ref
        {
            return Err(Refusal::Access.into());
        }
        self.external(&resource.owner_ref)?;
        Ok(())
    }
    /// Baseline eligibility may only narrow; every other policy field is immutable.
    fn policy(
        effective: &EffectiveConfiguration,
        baseline: &SourcePolicy,
    ) -> Result<(), WriteError> {
        let mut expected = baseline.clone();
        if effective.sources.len() != baseline.sources.len() {
            return Err(WriteError::Held);
        }
        for source in &mut expected.sources {
            let proposed = effective
                .policy
                .sources
                .iter()
                .find(|value| value.id == source.id)
                .ok_or(WriteError::Held)?;
            if !proposed
                .selected_profiles
                .iter()
                .all(|pin| source.selected_profiles.contains(pin))
            {
                return Err(WriteError::Held);
            }
            source
                .selected_profiles
                .clone_from(&proposed.selected_profiles);
        }
        if expected != effective.policy {
            return Err(WriteError::Held);
        }
        Ok(())
    }
    /// Baseline members are evidence, not arbitrary additional protected claims.
    fn protected(
        effective: &EffectiveConfiguration,
        checked: &CheckedPolicy,
    ) -> Result<(), WriteError> {
        let mut expected: BTreeMap<_, _> = checked
            .references()
            .iter()
            .map(|reference| (reference.id.clone(), reference.clone()))
            .collect();
        let extras = [
            "s1_embedding_model".to_owned(),
            "s1_tokenizer_qualification".to_owned(),
        ]
        .into_iter()
        .chain(
            checked
                .policy()
                .sources
                .iter()
                .map(|source| selection_key(&source.id)),
        );
        for key in extras {
            let pin = effective
                .protected_resources
                .get(&key)
                .ok_or(WriteError::Held)?;
            if expected.get(&key).is_some_and(|original| original != pin) {
                return Err(WriteError::Held);
            }
            expected.insert(key, pin.clone());
        }
        if expected != effective.protected_resources {
            return Err(WriteError::Held);
        }
        Ok(())
    }
    /// Stored N15 outcomes, never in-memory claims or first-profile fallback.
    fn selections(
        &self,
        effective: &EffectiveConfiguration,
        baseline: &SourcePolicy,
        qualified: &[Ref],
    ) -> Result<(), WriteError> {
        let expected: BTreeSet<_> = baseline
            .sources
            .iter()
            .map(|source| selection_key(&source.id))
            .collect();
        let actual: BTreeSet<_> = effective
            .protected_resources
            .keys()
            .filter(|key| key.starts_with("profile_selection."))
            .cloned()
            .collect();
        if expected != actual {
            return Err(WriteError::Held);
        }
        for source in &effective.policy.sources {
            let (pin, definition, processing) =
                effective.sources.get(&source.id).ok_or(WriteError::Held)?;
            let selection = effective
                .protected_resources
                .get(&selection_key(&source.id))
                .ok_or(WriteError::Held)?;
            let selected: ProfileSelection =
                storage::canonical_artifact(self.receipts, self.principal.id, selection)?;
            let ProfileSelection::Selected { profile, evidence } = selected else {
                return Err(WriteError::Held);
            };
            if profile != *pin
                || evidence.is_empty()
                || evidence.len() > 1000
                || !source.selected_profiles.contains(pin)
                || !qualified.contains(pin)
                || effective.profiles.get(&pin.id).is_none_or(|value| {
                    value.reference() != *pin || value.definition != *definition
                })
            {
                return Err(WriteError::Held);
            }
            for reference in &evidence {
                self.receipt_bytes(reference)?;
            }
            let defaults = &definition.processing;
            let expected = Processing {
                cleanup: effective
                    .selected
                    .0
                    .as_ref()
                    .unwrap_or(&defaults.cleanup)
                    .clone(),
                chunk: effective
                    .selected
                    .1
                    .as_ref()
                    .unwrap_or(&defaults.chunk)
                    .clone(),
                dedup: effective
                    .selected
                    .2
                    .as_ref()
                    .unwrap_or(&defaults.dedup)
                    .clone(),
            };
            if *processing != expected {
                return Err(WriteError::Held);
            }
        }
        Ok(())
    }
    /// Read external admission by role; qualification membership never recurses.
    pub(super) fn external(&self, reference: &Ref) -> Result<ImmutableResource, WriteError> {
        let value = self.source.read(reference, self.principal)?;
        if value.reference != *reference
            || value.bytes.len() > strict_json::MAX_BYTES
            || Digest::of(&value.bytes) != reference.digest
            || value.admission.digest != reference.digest
        {
            return Err(Refusal::Digest.into());
        }
        if value.admission.status != AdmissionStatus::Reviewed
            || value.admission.platform != self.principal.platform
        {
            return Err(WriteError::Held);
        }
        Ok(value)
    }
    /// Exact external approval of this receipt's immutable bytes.
    pub(super) fn qualification(
        &self,
        qualification: &Ref,
        artifact: &Ref,
    ) -> Result<(), WriteError> {
        let value = self.external(qualification)?;
        if !value.admission.references.contains(artifact) {
            return Err(WriteError::Held);
        }
        Ok(())
    }
}
