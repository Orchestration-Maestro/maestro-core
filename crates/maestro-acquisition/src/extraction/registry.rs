//! Pure replaceable registry adapter and core revalidation boundaries.
use super::{
    detect::{DetectionEvidence, Detector, id_valid},
    model::{ProfileDefinition, QualificationState, Registry},
};
pub use super::{
    model::Profile,
    outcome::{CheckedRegistry, HeldReason, ProfileSelection, RegistryUnavailable},
};
use crate::{
    policy::resolve::parse_resource,
    ports::{
        AdmissionStatus, CheckedPolicy, ImmutableResource, Principal, ProfileRegistry,
        ResourceSource, read_resource,
    },
    refusal::Refusal,
};
use maestro_kernel::scope::Scope;
use maestro_knowledge::collection::PolicyReference as Ref;
use std::collections::{BTreeMap, BTreeSet};

impl Profile {
    /// Profile refs bind canonical definition bytes, not JSON whitespace.
    #[must_use]
    pub fn reference(&self) -> Ref {
        Ref {
            id: self.definition.id.clone(),
            digest: self.definition_digest.clone(),
        }
    }
}
impl ProfileDefinition {
    /// Protected effective fields must match structurally, never by name alone.
    #[must_use]
    pub fn same_protected_fields(&self, other: &Self) -> bool {
        self.decode_limits == other.decode_limits
            && self.output_schema == other.output_schema
            && self.required_fidelity == other.required_fidelity
            && self.admission_rules == other.admission_rules
            && self.artifacts == other.artifacts
            && self.platforms == other.platforms
            && self.extractor == other.extractor
            && self.qualification_evidence == other.qualification_evidence
    }
}

/// Stable version-one definition preimage. Typed field order is frozen by tests.
///
/// # Errors
/// Serialization of an invalid typed definition refuses.
pub fn definition_bytes(definition: &ProfileDefinition) -> Result<Vec<u8>, Refusal> {
    let mut bytes = b"maestro-profile-definition/1\n".to_vec();
    bytes.extend(serde_json::to_vec(definition).map_err(|_| Refusal::Invalid)?);
    Ok(bytes)
}

/// Read-only local registry over any immutable resource adapter.
#[derive(Debug)]
pub struct LocalRegistry<'a>(&'a dyn ResourceSource);
impl<'a> LocalRegistry<'a> {
    /// Inject direct files or a substitute; callers and selection never change.
    #[must_use]
    pub fn new(source: &'a dyn ResourceSource) -> Self {
        Self(source)
    }
}
impl ProfileRegistry for LocalRegistry<'_> {
    fn resolve(
        &self,
        reference: &Ref,
        principal: &Principal<'_>,
    ) -> Result<CheckedRegistry, RegistryUnavailable> {
        resolve(self.0, reference, principal)
    }
    fn select(
        &self,
        checked: &CheckedRegistry,
        evidence: &DetectionEvidence,
        eligible: &[Ref],
    ) -> Result<ProfileSelection, RegistryUnavailable> {
        select(checked, evidence, eligible)
    }
}

/// Disabled registry returns the same explicit refusal from both operations.
#[derive(Debug)]
pub struct DisabledRegistry;
impl ProfileRegistry for DisabledRegistry {
    fn resolve(
        &self,
        _reference: &Ref,
        _principal: &Principal<'_>,
    ) -> Result<CheckedRegistry, RegistryUnavailable> {
        Err(RegistryUnavailable::Disabled)
    }
    fn select(
        &self,
        _checked: &CheckedRegistry,
        _evidence: &DetectionEvidence,
        _eligible: &[Ref],
    ) -> Result<ProfileSelection, RegistryUnavailable> {
        Err(RegistryUnavailable::Disabled)
    }
}

/// Core policy binding; a substitute cannot return another registry/principal.
///
/// # Errors
/// Disabled, missing, corrupt, inaccessible or unqualified registries refuse.
pub fn checked_resolve(
    adapter: &dyn ProfileRegistry,
    reference: &Ref,
    principal: &Principal<'_>,
    policy: &CheckedPolicy,
) -> Result<CheckedRegistry, RegistryUnavailable> {
    let mut checked = adapter.resolve(reference, principal)?;
    let resource = &checked.registry.resource;
    let baseline = &policy.policy().resource;
    if reference != &policy.policy().profiles
        || checked.reference != *reference
        || checked.principal != (principal.id.to_owned(), principal.platform.to_owned())
        || resource.collection_id != baseline.collection_id
        || resource.visibility != baseline.visibility
        || resource.scope_tags != baseline.scope_tags
        || !resource.scope_tags.iter().all(|tag| {
            tag.parse::<Scope>()
                .is_ok_and(|scope| principal.scopes.covers(&scope))
        })
    {
        return Err(RegistryUnavailable::Access);
    }
    checked.eligible = policy
        .policy()
        .sources
        .iter()
        .flat_map(|source| source.selected_profiles.clone())
        .collect();
    Ok(checked)
}

/// Core checks substitute selection against the immutable deterministic outcome.
///
/// # Errors
/// Disabled adapters, forged refs/reasons/partial output or ineligible refs refuse.
pub fn checked_select(
    adapter: &dyn ProfileRegistry,
    checked: &CheckedRegistry,
    evidence: &DetectionEvidence,
    eligible: &[Ref],
) -> Result<ProfileSelection, RegistryUnavailable> {
    let returned = adapter.select(checked, evidence, eligible)?;
    let expected = select(checked, evidence, eligible)?;
    if returned != expected
        || !eligible
            .iter()
            .all(|reference| checked.eligible.contains(reference))
    {
        return Err(RegistryUnavailable::Corrupt);
    }
    Ok(returned)
}

/// Resolve exact resources once, digest-checking even unqualified profile bytes.
struct Resources<'a> {
    /// Read-only injected port.
    source: &'a dyn ResourceSource,
    /// Current caller context.
    principal: &'a Principal<'a>,
    /// Immutable closure, bounded to 1,000 logical IDs.
    cache: BTreeMap<String, ImmutableResource>,
}
impl Resources<'_> {
    /// Shared byte verification; qualification is checked separately by role.
    fn read(&mut self, reference: &Ref) -> Result<ImmutableResource, RegistryUnavailable> {
        Ok(read_resource(
            self.source,
            self.principal,
            &mut self.cache,
            reference,
            &|_| Ok(()),
        )?)
    }
    /// External reviewed qualification, never granted by wire state.
    fn own_qualified(&self, resource: &ImmutableResource) -> bool {
        resource.admission.status == AdmissionStatus::Reviewed
            && resource.admission.platform == self.principal.platform
    }
    /// Check the entire byte-verified closure once, including cycles and diamonds.
    /// Both sets are bounded by the read cache's 1,000 logical IDs.
    fn qualified(&self, resource: &ImmutableResource) -> bool {
        let mut pending = BTreeSet::from([resource.reference.id.as_str()]);
        let mut visited = BTreeSet::new();
        while let Some(id) = pending.pop_first() {
            visited.insert(id);
            let Some(member) = self.cache.get(id) else {
                return false;
            };
            if !self.own_qualified(member) {
                return false;
            }
            pending.extend(
                member
                    .admission
                    .references
                    .iter()
                    .map(|reference| reference.id.as_str())
                    .filter(|id| !visited.contains(id)),
            );
        }
        true
    }
}

/// Validate strict registry ownership, definitions and exact evidence closure.
fn resolve(
    source: &dyn ResourceSource,
    reference: &Ref,
    principal: &Principal<'_>,
) -> Result<CheckedRegistry, RegistryUnavailable> {
    let mut resources = Resources {
        source,
        principal,
        cache: BTreeMap::new(),
    };
    let immutable = resources.read(reference)?;
    if !resources.own_qualified(&immutable) {
        return Err(RegistryUnavailable::Unqualified);
    }
    let registry: Registry = parse_resource(&immutable.bytes)?;
    if registry.resource.id != reference.id {
        return Err(RegistryUnavailable::Corrupt);
    }
    // Inventory profiles may be held; all other root dependencies must qualify.
    for dependency in &immutable.admission.references {
        if !registry
            .profiles
            .iter()
            .any(|profile| profile.reference() == *dependency)
        {
            let member = resources.read(dependency)?;
            if !resources.qualified(&member) {
                return Err(RegistryUnavailable::Unqualified);
            }
        }
    }
    if principal.id.is_empty()
        || registry.resource.scope_tags.is_empty()
        || !registry.resource.scope_tags.iter().all(|tag| {
            tag.parse::<Scope>()
                .is_ok_and(|scope| principal.scopes.covers(&scope))
        })
    {
        return Err(RegistryUnavailable::Access);
    }
    for required in [&registry.resource.owner_ref, &registry.qualification] {
        let resource = resources.read(required)?;
        if !resources.qualified(&resource) {
            return Err(RegistryUnavailable::Unqualified);
        }
    }
    let mut ids = BTreeSet::new();
    let mut qualified = Vec::new();
    for profile in &registry.profiles {
        if !ids.insert(&profile.definition.id) {
            return Err(RegistryUnavailable::Corrupt);
        }
        validate_definition(&profile.definition)?;
        let bytes = definition_bytes(&profile.definition)?;
        let member = resources.read(&profile.reference())?;
        if member.bytes != bytes {
            return Err(RegistryUnavailable::Corrupt);
        }
        let admitted = qualify_profile(&mut resources, profile)?;
        if resources.qualified(&member) && admitted {
            qualified.push(profile.reference());
        }
    }
    if !qualified.contains(&registry.unknown_profile) {
        return Err(RegistryUnavailable::Unqualified);
    }
    Ok(CheckedRegistry {
        reference: reference.clone(),
        registry,
        principal: (principal.id.into(), principal.platform.into()),
        qualified,
        eligible: vec![],
    })
}

/// Bounded shape requirements independent of parser global resource limits.
fn validate_definition(profile: &ProfileDefinition) -> Result<(), RegistryUnavailable> {
    if !id_valid(&profile.id) || !id_valid(&profile.processing.chunk) {
        return Err(RegistryUnavailable::Corrupt);
    }
    for values in [
        &profile.languages,
        &profile.required_fidelity,
        &profile.admission_rules,
        &profile.platforms,
        &profile.processing.cleanup,
        &profile.processing.dedup,
    ] {
        if values.len() > 1000 || !values.iter().all(|value| id_valid(value)) {
            return Err(RegistryUnavailable::Corrupt);
        }
    }
    if profile.detectors.len() > 1000 || !profile.detectors.iter().all(Detector::valid) {
        return Err(RegistryUnavailable::Corrupt);
    }
    let mut nodes = 0;
    if !profile
        .structures
        .iter()
        .all(|structure| structure.validate(1, &mut nodes))
    {
        return Err(RegistryUnavailable::Corrupt);
    }
    for refs in [&profile.artifacts, &profile.qualification_evidence] {
        if refs.len() > 1000 {
            return Err(RegistryUnavailable::Corrupt);
        }
    }
    Ok(())
}

/// Qualification requires gold, approved thresholds and pinned installed artifacts.
fn qualify_profile(
    resources: &mut Resources<'_>,
    profile: &Profile,
) -> Result<bool, RegistryUnavailable> {
    let definition = &profile.definition;
    let mut admitted = definition.qualification_state == QualificationState::Qualified
        && definition
            .platforms
            .iter()
            .any(|platform| platform == resources.principal.platform)
        && !definition.required_fidelity.is_empty()
        && !definition.admission_rules.is_empty()
        && definition.artifacts.contains(&definition.extractor);
    let mut evidence_capabilities = Vec::new();
    for reference in &definition.qualification_evidence {
        let value = resources.read(reference)?;
        admitted &= resources.qualified(&value);
        evidence_capabilities.extend(value.admission.capabilities);
    }
    admitted &= evidence_capabilities.contains(&"gold".into())
        && evidence_capabilities.contains(&"thresholds".into());
    let output = resources.read(&definition.output_schema)?;
    admitted &= resources.qualified(&output);
    let mut installed = Vec::new();
    for reference in &definition.artifacts {
        let artifact = resources.read(reference)?;
        admitted &= resources.qualified(&artifact);
        if reference == &definition.extractor {
            installed = artifact.admission.capabilities;
        }
    }
    for detector in &definition.detectors {
        if let Detector::ParserCapability { id } = detector {
            admitted &= installed.contains(id);
        }
    }
    Ok(admitted)
}

/// Deterministic conservative selection; contradictory candidates never win ties.
fn select(
    checked: &CheckedRegistry,
    evidence: &DetectionEvidence,
    eligible: &[Ref],
) -> Result<ProfileSelection, RegistryUnavailable> {
    if eligible.len() > 1000
        || !eligible
            .iter()
            .all(|reference| checked.profile(reference).is_some())
    {
        return Err(RegistryUnavailable::Corrupt);
    }
    let input = evidence.input();
    let mut candidates = Vec::new();
    if !input.encrypted && !input.malformed {
        for profile in &checked.registry.profiles {
            let definition = &profile.definition;
            let reference = profile.reference();
            let content = definition.detectors.iter().any(Detector::substantive)
                || !definition.structures.is_empty();
            if reference != checked.registry.unknown_profile
                && eligible.contains(&reference)
                && checked.qualified.contains(&reference)
                && content
                && definition
                    .detectors
                    .iter()
                    .all(|detector| detector.detects(evidence))
                && definition
                    .structures
                    .iter()
                    .all(|structure| structure.detects(evidence))
            {
                candidates.push(reference);
            }
        }
    }
    if let [profile] = candidates.as_slice() {
        return Ok(ProfileSelection::Selected {
            profile: profile.clone(),
            evidence: input.evidence.clone(),
        });
    }
    let reason = if candidates.len() > 1 {
        HeldReason::Ambiguous
    } else {
        HeldReason::Unknown
    };
    Ok(ProfileSelection::Held {
        reason,
        safe_profile: checked.registry.unknown_profile.clone(),
        evidence: input.evidence.clone(),
        partial: evidence.partial().clone(),
    })
}
