//! Small read-only ports; authority and transport startup are deliberately absent.
use crate::{
    extraction::{
        detect::DetectionEvidence,
        outcome::{CheckedRegistry, ProfileSelection, RegistryUnavailable},
    },
    policy::{
        acquisition::AcquisitionProfile,
        decisions::{Decisions, Promotion},
        identity::IdentityMigration,
        schema::SourcePolicy,
        shape::valid_id,
    },
    refusal::Refusal,
    transport::address::AddressTable,
};
use maestro_kernel::{artifact::Digest, scope::ScopeSet};
use maestro_knowledge::collection::Declaration;
use maestro_knowledge::{collection::PolicyReference as Ref, strict_json::MAX_BYTES};
use std::collections::BTreeMap;
use std::fmt::Debug;

/// Current principal context, supplied by the caller, never by policy JSON.
#[derive(Debug)]
pub struct Principal<'a> {
    /// Authenticated logical principal name.
    pub id: &'a str,
    /// Host platform whose qualification is being checked.
    pub platform: &'a str,
    /// Fresh current kernel read grants, not acquisition entitlement.
    pub scopes: &'a ScopeSet,
}

/// Reviewed admission is external evidence, not a self-granted policy field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionStatus {
    /// Exact resource admitted by the reviewed local/catalog baseline.
    Reviewed,
    /// A review-only proposal, never executable.
    Proposed,
    /// Qualification is incomplete or failed.
    Held,
    /// Previously admitted evidence is revoked.
    Revoked,
}

/// Trusted summary of the not-yet-implemented qualification/declaration schemas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admission {
    /// Exact digest reviewed, including artifact/runtime declaration bytes.
    pub digest: Digest,
    /// Exact qualified host platform.
    pub platform: String,
    /// Admitted capability IDs, including supported transport names.
    pub capabilities: Vec<String>,
    /// Only reviewed resources admit.
    pub status: AdmissionStatus,
    /// Exact eligible extraction-profile members of an immutable registry.
    pub references: Vec<Ref>,
}

/// One immutable resource plus separate, caller-authenticated admission evidence.
#[derive(Debug, Clone)]
pub struct ImmutableResource {
    /// Exact logical identity and digest requested.
    pub reference: Ref,
    /// Original bytes; hashing never ignores whitespace or changes the baseline.
    pub bytes: Vec<u8>,
    /// Reviewed evidence supplied outside the untrusted wire document.
    pub admission: Admission,
}

/// Replaceable local or catalog resource reader, with no effects beyond reads.
pub trait ResourceSource: Debug {
    /// Read one resource; the core rechecks the returned identity and digest.
    ///
    /// # Errors
    /// Missing, disabled, unreadable or unauthorized resources refuse.
    fn read(
        &self,
        reference: &Ref,
        principal: &Principal<'_>,
    ) -> Result<ImmutableResource, Refusal>;
}

/// Cache exact bytes once per logical ID; admission checks remain caller-owned.
pub(crate) fn read_resource(
    source: &dyn ResourceSource,
    principal: &Principal<'_>,
    cache: &mut BTreeMap<String, ImmutableResource>,
    reference: &Ref,
    review: &impl Fn(&ImmutableResource) -> Result<(), Refusal>,
) -> Result<ImmutableResource, Refusal> {
    if !valid_id(&reference.id) {
        return Err(Refusal::Invalid);
    }
    if let Some(resource) = cache.get(&reference.id) {
        return if resource.reference == *reference {
            Ok(resource.clone())
        } else {
            Err(Refusal::Digest)
        };
    }
    if cache.len() >= 1000 {
        return Err(Refusal::Invalid);
    }
    let resource = source.read(reference, principal)?;
    if resource.reference != *reference
        || resource.bytes.len() > MAX_BYTES
        || Digest::of(&resource.bytes) != reference.digest
        || resource.admission.digest != reference.digest
    {
        return Err(Refusal::Digest);
    }
    review(&resource)?;
    cache.insert(reference.id.clone(), resource.clone());
    for member in &resource.admission.references {
        read_resource(source, principal, cache, member, review)?;
    }
    Ok(resource)
}

/// Read-only source-policy resolution; no installation/activation side effects.
pub trait PolicySource: Debug {
    /// Resolve one collection through the same strict core validator.
    ///
    /// # Errors
    /// Missing, corrupt, unreviewed, unqualified or contradictory input refuses.
    fn resolve(
        &self,
        collection: &Declaration,
        principal: &Principal<'_>,
    ) -> Result<CheckedPolicy, Refusal>;
}
/// Replaceable pure profile resolution/selection, with no acquisition or launches.
/// Consumers use the core checked wrappers to revalidate substitute outcomes.
pub trait ProfileRegistry: Debug {
    /// Resolve exact immutable definitions and their qualification closure.
    ///
    /// # Errors
    /// Disabled, missing, corrupt or unqualified registries refuse.
    fn resolve(
        &self,
        reference: &Ref,
        principal: &Principal<'_>,
    ) -> Result<CheckedRegistry, RegistryUnavailable>;
    /// Select one content-backed eligible profile or retain an explicit held result.
    ///
    /// # Errors
    /// Disabled registries or invalid eligible references refuse without fallback.
    fn select(
        &self,
        checked: &CheckedRegistry,
        evidence: &DetectionEvidence,
        eligible: &[Ref],
    ) -> Result<ProfileSelection, RegistryUnavailable>;
}
/// A read-only digest-bound policy; obtaining it creates no authority or effects.
#[derive(Debug)]
pub struct CheckedPolicy {
    /// Sorted, deduplicated immutable members of the validated baseline closure.
    pub(crate) references: Vec<Ref>,
    /// Exact original immutable baseline reference.
    pub(crate) reference: Ref,
    /// Reviewed address data compiled with the non-removable denial floor.
    pub(crate) address_table: AddressTable,
    /// Checked source-policy contract.
    pub(crate) policy: SourcePolicy,
    /// Resolved acquisition profiles keyed by logical ID.
    pub(crate) acquisition_profiles: BTreeMap<String, AcquisitionProfile>,
    /// Resolved reviewed denials and content dispositions.
    pub(crate) decisions: Vec<Decisions>,
    /// Resolved separate promotions, keyed by source namespace.
    pub(crate) promotions: BTreeMap<String, Vec<Promotion>>,
    /// Explicit reviewed old-to-new mappings; resolution never applies them.
    pub(crate) identity_migrations: BTreeMap<String, IdentityMigration>,
}
impl CheckedPolicy {
    /// Exact currently validated baseline closure, sorted by reference ID.
    /// There is no setter or unchecked construction path.
    #[must_use]
    pub fn references(&self) -> &[Ref] {
        &self.references
    }
    /// Immutable destination classifier; data cannot relax its denial floor.
    #[must_use]
    pub fn address_table(&self) -> &AddressTable {
        &self.address_table
    }
    /// Original policy digest, never a digest of reformatted bytes.
    #[must_use]
    pub fn reference(&self) -> &Ref {
        &self.reference
    }
    /// Checked source declarations, not permission to fetch.
    #[must_use]
    pub fn policy(&self) -> &SourcePolicy {
        &self.policy
    }
    /// Persisted reviewed mappings; callers must not silently rewrite fetch keys.
    #[must_use]
    pub fn identity_migrations(&self) -> &BTreeMap<String, IdentityMigration> {
        &self.identity_migrations
    }
    /// Exact profile members; callers cannot mutate transport/readiness selection.
    #[must_use]
    pub fn acquisition_profiles(&self) -> &BTreeMap<String, AcquisitionProfile> {
        &self.acquisition_profiles
    }
}

#[cfg(test)]
mod mutation_tests {
    use super::{
        Admission, AdmissionStatus, ImmutableResource, Principal, Ref, ResourceSource,
        read_resource,
    };
    use crate::Refusal;
    use maestro_kernel::{artifact::Digest, store::Database};
    use maestro_knowledge::strict_json::MAX_BYTES;
    use std::{collections::BTreeMap, fs};
    #[derive(Debug)]
    struct Source(ImmutableResource);
    impl ResourceSource for Source {
        fn read(&self, _: &Ref, _: &Principal<'_>) -> Result<ImmutableResource, Refusal> {
            Ok(self.0.clone())
        }
    }
    #[test]
    fn s6t_resource_exact_byte_ceiling_is_admitted() {
        let bytes = vec![b'x'; MAX_BYTES];
        let digest = Digest::of(&bytes);
        let reference = Ref {
            id: "boundary".into(),
            digest: digest.clone(),
        };
        let source = Source(ImmutableResource {
            reference: reference.clone(),
            bytes,
            admission: Admission {
                digest,
                platform: "synthetic".into(),
                capabilities: vec![],
                status: AdmissionStatus::Reviewed,
                references: vec![],
            },
        });
        let root = maestro_test_scratch::scratch_directory().unwrap();
        let database = Database::open_in(&root).unwrap();
        let scopes = database.visible("reader").unwrap();
        let principal = Principal {
            id: "reader",
            platform: "synthetic",
            scopes: &scopes,
        };
        assert_eq!(
            read_resource(
                &source,
                &principal,
                &mut BTreeMap::new(),
                &reference,
                &|_| Ok(())
            )
            .unwrap()
            .bytes
            .len(),
            MAX_BYTES
        );
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
