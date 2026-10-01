//! Verified whole-set payloads and scope links for the shared snapshot reader.
use super::{ProcessingSnapshot, ResolvedSnapshot, SnapshotReader};
use crate::{
    Ref, Refusal,
    adaptation::{
        artifacts::{CleanupRules, DedupKeys, S1ChunkStrategy},
        change::EffectiveConfiguration,
        manifest::WriteError,
        storage,
    },
    extraction::{model::Processing, outcome::ProfileSelection},
    policy::{
        decisions::{Action, Decision, Decisions},
        schema::SourcePolicy,
    },
};
use maestro_kernel::{acquisition::Handle, artifact::Digest, gateway::ModelCard};
use maestro_knowledge::strict_json;
use serde::de::IgnoredAny;
use std::collections::{BTreeMap, BTreeSet};
impl SnapshotReader<'_> {
    /// Every whole-set selection resolves only through its matching group schema.
    pub(super) fn payloads(
        &self,
        resolved: &mut ResolvedSnapshot,
        baseline: &SourcePolicy,
    ) -> Result<(), WriteError> {
        let effective = &resolved.snapshot.effective;
        let model = effective
            .protected_resources
            .get("s1_embedding_model")
            .ok_or(WriteError::Held)?;
        let tokenizer = effective
            .protected_resources
            .get("s1_tokenizer_qualification")
            .ok_or(WriteError::Held)?;
        let bytes = self.external(model)?.bytes;
        let _: IgnoredAny = strict_json::parse(&bytes).map_err(|_| WriteError::Held)?;
        let card = ModelCard::from_json_bytes(&bytes).map_err(|_| WriteError::Held)?;
        self.external(tokenizer)?;
        let mut tokens = BTreeMap::new();
        for (id, reference) in &effective.approved_cleanup {
            Self::key(id, reference)?;
            let value: CleanupRules =
                storage::canonical_artifact(self.receipts, self.principal.id, reference)?;
            self.resource(&value.resource, baseline)?;
            self.qualification(&value.qualification, reference)?;
            resolved.cleanup.insert(id.clone(), value);
        }
        for (id, reference) in &effective.approved_chunks {
            Self::key(id, reference)?;
            let value: S1ChunkStrategy =
                storage::canonical_artifact(self.receipts, self.principal.id, reference)?;
            self.resource(&value.resource, baseline)?;
            self.qualification(&value.qualification, reference)?;
            value.validate(&card, model, tokenizer)?;
            tokens.insert(id.clone(), value.hard_max_tokens.get());
            resolved.chunks.insert(id.clone(), value);
        }
        for (id, reference) in &effective.approved_dedup {
            Self::key(id, reference)?;
            let value: DedupKeys =
                storage::canonical_artifact(self.receipts, self.principal.id, reference)?;
            self.resource(&value.resource, baseline)?;
            self.qualification(&value.qualification, reference)?;
            value.validate()?;
            resolved.dedup.insert(id.clone(), value);
        }
        if tokens != effective.qualified_chunk_tokens
            || effective.qualified_model_limit
                != u64::from(card.fields().limits.context_tokens.get())
        {
            return Err(WriteError::Held);
        }
        for profile in effective.profiles.values() {
            Self::processing(effective, &profile.definition.processing)?;
        }
        for (_, _, processing) in effective.sources.values() {
            Self::processing(effective, processing)?;
        }
        for (id, reference) in &effective.protected_resources {
            if !id.starts_with("profile_selection.") {
                self.external(reference)?;
            }
        }
        Ok(())
    }
    /// Group-map keys cannot smuggle a substituted Ref or a different schema.
    fn key(id: &str, reference: &Ref) -> Result<(), WriteError> {
        if id != reference.id {
            return Err(WriteError::Held);
        }
        Ok(())
    }
    /// Profile defaults and effective overrides both require exact approved pins.
    fn processing(
        effective: &EffectiveConfiguration,
        processing: &Processing,
    ) -> Result<(), WriteError> {
        for (map, pin) in [
            (&effective.approved_cleanup, &processing.cleanup),
            (&effective.approved_chunks, &processing.chunk),
            (&effective.approved_dedup, &processing.dedup),
        ] {
            if map.get(&pin.id) != Some(pin) {
                return Err(WriteError::Held);
            }
        }
        Ok(())
    }
    /// Existing registries stay external; new exclusion artifacts are singletons.
    pub(super) fn decisions(
        &self,
        effective: &EffectiveConfiguration,
        baseline: &SourcePolicy,
    ) -> Result<(), WriteError> {
        let mut expected = BTreeMap::new();
        for reference in &baseline.registries {
            let registry: Decisions = storage::decode(&self.external(reference)?.bytes)?;
            for entry in registry.entries {
                expected.insert(entry.id.clone(), (reference.clone(), entry));
            }
        }
        for (id, (reference, entry)) in &effective.decisions {
            let value = (reference.clone(), entry.clone());
            if expected.get(id).is_some_and(|original| *original != value) {
                return Err(WriteError::Held);
            }
            if expected.contains_key(id) {
                continue;
            }
            self.singleton(id, reference, entry, baseline)?;
        }
        if !expected
            .keys()
            .all(|key| effective.decisions.contains_key(key))
        {
            return Err(WriteError::Held);
        }
        Ok(())
    }
    /// New decisions use N03's singleton envelope, not a parallel entry model.
    fn singleton(
        &self,
        id: &str,
        reference: &Ref,
        entry: &Decision,
        baseline: &SourcePolicy,
    ) -> Result<(), WriteError> {
        let singleton: Decisions =
            storage::canonical_artifact(self.receipts, self.principal.id, reference)?;
        self.resource(&singleton.resource, baseline)?;
        if singleton.entries != [entry.clone()]
            || entry.id != id
            || entry.action == Action::DenyFetch
            || entry.evidence.is_empty()
            || entry.evidence.len() > 1000
        {
            return Err(WriteError::Held);
        }
        self.qualification(&singleton.qualification, reference)?;
        self.external(&entry.authority)?;
        for evidence in &entry.evidence {
            self.external(evidence)?;
        }
        Ok(())
    }
    /// Digest-check evidence bytes without inventing an evidence wire schema.
    pub(super) fn receipt_bytes(&self, reference: &Ref) -> Result<(), WriteError> {
        let handle = reference.id.parse().map_err(|_| Refusal::Invalid)?;
        let value = self
            .receipts
            .read(self.principal.id, handle)
            .map_err(|_| WriteError::Storage)?
            .ok_or(Refusal::Access)?;
        if Digest::of(value.bytes()) != reference.digest {
            return Err(Refusal::Digest.into());
        }
        Ok(())
    }
    /// Link every pinned receipt and selection evidence; external pins stay external.
    pub(super) fn links(&self, snapshot: &ProcessingSnapshot) -> Result<Vec<Handle>, WriteError> {
        let effective = &snapshot.effective;
        let mut links = BTreeSet::new();
        for map in [
            &effective.approved_cleanup,
            &effective.approved_chunks,
            &effective.approved_dedup,
        ] {
            for reference in map.values() {
                links.insert(reference.id.parse().map_err(|_| Refusal::Invalid)?);
            }
        }
        for (key, pin) in &effective.protected_resources {
            if !key.starts_with("profile_selection.") {
                continue;
            }
            links.insert(pin.id.parse().map_err(|_| Refusal::Invalid)?);
            let selection: ProfileSelection =
                storage::canonical_artifact(self.receipts, self.principal.id, pin)?;
            let ProfileSelection::Selected { evidence, .. } = selection else {
                return Err(WriteError::Held);
            };
            for pin in evidence {
                links.insert(pin.id.parse().map_err(|_| Refusal::Invalid)?);
            }
        }
        for (pin, _) in effective.decisions.values() {
            if !effective.policy.registries.contains(pin) {
                links.insert(pin.id.parse().map_err(|_| Refusal::Invalid)?);
            }
        }
        Ok(links.into_iter().collect())
    }
}
