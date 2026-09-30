//! One strict validator for local files and immutable catalog resources.
use super::{
    acquisition::{AcquisitionProfile, Transport},
    checks,
    decisions::{Decision, Decisions, Promotions},
    resource::Resource,
    schema::SourcePolicy,
    shape,
    source::{Discovery, Source},
};
use crate::{
    ports::{AdmissionStatus, CheckedPolicy, ImmutableResource, Principal, ResourceSource},
    refusal::Refusal,
};
use maestro_kernel::artifact::Digest;
use maestro_knowledge::collection::PolicyReference as Ref;
use maestro_knowledge::{
    collection::{Declaration, Schema},
    strict_json,
};
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};

/// Decode a bounded strict source-policy document, then validate its local shape.
///
/// # Errors
/// Duplicate/unknown keys, wrong object shapes, over-limit input or invalid fields.
pub fn parse_policy(text: &str) -> Result<SourcePolicy, Refusal> {
    let policy = parse_resource(text.as_bytes())?;
    checks::policy(&policy)?;
    Ok(policy)
}

/// Decode any of the plan's typed resources with the same defensive limits.
///
/// # Errors
/// Non-object, duplicate, unknown, missing, malformed or over-limit fields refuse.
pub fn parse_resource<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Refusal> {
    strict_json::parse(bytes).map_err(|_| Refusal::Invalid)
}

/// Resolve all exact references through a single validator; never start effects.
///
/// # Errors
/// Import-only links, missing resources, changed digests, unreviewed evidence,
/// unsupported capabilities, inaccessible scopes and contradictions refuse.
pub fn validate(
    source: &dyn ResourceSource,
    collection: &Declaration,
    principal: &Principal<'_>,
) -> Result<CheckedPolicy, Refusal> {
    if collection.schema != Schema::V2 {
        return Err(Refusal::ImportOnly);
    }
    let reference = collection
        .source_policy
        .as_ref()
        .ok_or(Refusal::ImportOnly)?;
    let mut closure = Closure {
        source,
        principal,
        resources: BTreeMap::new(),
    };
    let immutable = closure.read(reference)?;
    let policy: SourcePolicy = parse_resource(&immutable.bytes)?;
    closure.resource(&policy.resource, collection)?;
    if policy.resource.id != reference.id {
        return Err(Refusal::Invalid);
    }
    checks::policy(&policy)?;
    for required in [
        &policy.profiles,
        &policy.address_table,
        &policy.retention_rule,
        &policy.qualification,
        &policy.adaptation.matrix,
        &policy.adaptation.thresholds,
        &policy.adaptation.baseline,
    ] {
        closure.read(required)?;
    }
    let extraction = closure.read(&policy.profiles)?;
    let profiles = read_profiles(&mut closure, &policy, collection)?;
    let registries = read_registries(&mut closure, &policy, collection)?;
    for source in &policy.sources {
        if !policy
            .acquisition_profiles
            .contains(&source.acquisition_profile)
        {
            return Err(Refusal::Digest);
        }
        let profile = profiles
            .get(&source.acquisition_profile.id)
            .ok_or(Refusal::Missing)?;
        checks::profile(
            profile,
            source
                .limits
                .elapsed_ms
                .get()
                .min(policy.aggregate_limits.elapsed_ms.get()),
        )?;
        for reference in &source.selected_profiles {
            if !extraction.admission.references.contains(reference) {
                return Err(Refusal::Digest);
            }
            closure.read(reference)?;
        }
        for reference in &source.decisions {
            if !policy.registries.contains(reference) {
                return Err(Refusal::Missing);
            }
        }
        for discovery in &source.discovery {
            if let Discovery::Api { mapping } = discovery {
                closure.read(mapping)?;
            }
        }
        check_promotions(&mut closure, source, collection)?;
        if let Some(reference) = source.connector.as_ref().or(source.wiki_mapping.as_ref()) {
            closure.read(reference)?;
            // N43/N47 own connector/wiki execution: evidence cannot enable it.
            return Err(Refusal::Unsupported);
        }
        for reference in source
            .robots
            .r#override
            .iter()
            .chain(source.identity.migration.iter())
        {
            closure.read(reference)?;
        }
    }
    validate_decisions(&registries, &policy)?;
    Ok(CheckedPolicy {
        reference: reference.clone(),
        policy,
        acquisition_profiles: profiles,
    })
}

/// One request's immutable closure; each logical ID resolves at most once.
struct Closure<'a> {
    /// Injected read-only source, never a connector host.
    source: &'a dyn ResourceSource,
    /// Current principal and platform supplied outside configuration.
    principal: &'a Principal<'a>,
    /// Cached exact references, refusing a second digest under the same ID.
    resources: BTreeMap<String, ImmutableResource>,
}
impl Closure<'_> {
    /// Read and validate the exact immutable bytes and external admission summary.
    fn read(&mut self, reference: &Ref) -> Result<ImmutableResource, Refusal> {
        if !shape::valid_id(&reference.id) {
            return Err(Refusal::Invalid);
        }
        if let Some(resource) = self.resources.get(&reference.id) {
            return if &resource.reference == reference {
                Ok(resource.clone())
            } else {
                Err(Refusal::Digest)
            };
        }
        if self.resources.len() >= 1000 {
            return Err(Refusal::Invalid);
        }
        let resource = self.source.read(reference, self.principal)?;
        if &resource.reference != reference
            || resource.bytes.len() > strict_json::MAX_BYTES
            || Digest::of(&resource.bytes) != reference.digest
            || resource.admission.digest != reference.digest
        {
            return Err(Refusal::Digest);
        }
        if resource.admission.status != AdmissionStatus::Reviewed
            || resource.admission.platform != self.principal.platform
        {
            return Err(Refusal::Unqualified);
        }
        self.resources
            .insert(reference.id.clone(), resource.clone());
        for member in &resource.admission.references {
            self.read(member)?;
        }
        Ok(resource)
    }
    /// Common typed resource fields and owner evidence.
    fn resource<S>(
        &mut self,
        resource: &Resource<S>,
        collection: &Declaration,
    ) -> Result<(), Refusal> {
        checks::resource(resource, collection, self.principal)?;
        self.read(&resource.owner_ref)?;
        Ok(())
    }
}

/// Read every required exclusion registry, including immutable evidence closure.
fn read_registries(
    closure: &mut Closure<'_>,
    policy: &SourcePolicy,
    collection: &Declaration,
) -> Result<Vec<Decisions>, Refusal> {
    let mut registries = Vec::new();
    let mut ids = BTreeSet::new();
    for reference in &policy.registries {
        let immutable = closure.read(reference)?;
        let registry: Decisions = parse_resource(&immutable.bytes)?;
        closure.resource(&registry.resource, collection)?;
        if registry.resource.id != reference.id {
            return Err(Refusal::Invalid);
        }
        closure.read(&registry.qualification)?;
        for decision in &registry.entries {
            if !ids.insert(decision.id.clone()) || decision.evidence.is_empty() {
                return Err(Refusal::Invalid);
            }
            closure.read(&decision.authority)?;
            for evidence in &decision.evidence {
                closure.read(evidence)?;
            }
            if let Some(reference) = &decision.reversal {
                closure.read(reference)?;
            }
        }
        registries.push(registry);
    }
    Ok(registries)
}

/// Contradictory dispositions never acquire implicit ordering or precedence.
fn validate_decisions(registries: &[Decisions], policy: &SourcePolicy) -> Result<(), Refusal> {
    let mut entries = Vec::new();
    for registry in registries {
        for decision in &registry.entries {
            let source = policy
                .sources
                .iter()
                .find(|source| source.id == decision.selector.source_id)
                .ok_or(Refusal::Invalid)?;
            checks::selector(&decision.selector, source)?;
            check_conflicts(&entries, decision)?;
            entries.push(decision);
        }
    }
    Ok(())
}

/// Resolve installed profile members without choosing a default transport.
fn read_profiles(
    closure: &mut Closure<'_>,
    policy: &SourcePolicy,
    collection: &Declaration,
) -> Result<BTreeMap<String, AcquisitionProfile>, Refusal> {
    let mut profiles = BTreeMap::new();
    for reference in &policy.acquisition_profiles {
        let immutable = closure.read(reference)?;
        let profile: AcquisitionProfile = parse_resource(&immutable.bytes)?;
        closure.resource(&profile.resource, collection)?;
        if profile.resource.id != reference.id {
            return Err(Refusal::Invalid);
        }
        checks::profile(&profile, policy.aggregate_limits.elapsed_ms.get())?;
        let adapter = closure.read(&profile.adapter)?;
        closure.read(&profile.qualification)?;
        let transport = match profile.transport {
            Transport::Http => "http",
            Transport::BrowserRequest => "browser_request",
            Transport::BrowserRender => "browser_render",
        };
        if !adapter
            .admission
            .capabilities
            .iter()
            .any(|capability| capability == transport)
            || !profile
                .required_capabilities
                .iter()
                .all(|capability| adapter.admission.capabilities.contains(capability))
        {
            return Err(Refusal::Unsupported);
        }
        profiles.insert(reference.id.clone(), profile);
    }
    Ok(profiles)
}

/// Promotions retain their own evidence and never override fetch denial.
fn check_promotions(
    closure: &mut Closure<'_>,
    source: &Source,
    collection: &Declaration,
) -> Result<(), Refusal> {
    for reference in &source.promotions {
        let immutable = closure.read(reference)?;
        let promotions: Promotions = parse_resource(&immutable.bytes)?;
        closure.resource(&promotions.resource, collection)?;
        if promotions.resource.id != reference.id {
            return Err(Refusal::Invalid);
        }
        closure.read(&promotions.qualification)?;
        checks::unique(promotions.entries.iter().map(|entry| entry.id.as_str()))?;
        for entry in &promotions.entries {
            checks::selector(&entry.selector, source)?;
            if entry.evidence.is_empty() {
                return Err(Refusal::Invalid);
            }
            closure.read(&entry.authority)?;
            for evidence in &entry.evidence {
                closure.read(evidence)?;
            }
            if let Some(reference) = &entry.reversal {
                closure.read(reference)?;
            }
        }
    }
    Ok(())
}

/// Refuse incompatible dispositions over the same declarative selector.
fn check_conflicts(entries: &[&Decision], decision: &Decision) -> Result<(), Refusal> {
    // ponytail: quadratic comparisons under the 20,000-item parser ceiling;
    // index by source/selector if reviewed registries grow enough to need it.
    for previous in entries {
        if checks::overlap(&previous.selector, &decision.selector)
            && previous.action != decision.action
        {
            return Err(Refusal::Invalid);
        }
    }
    Ok(())
}
