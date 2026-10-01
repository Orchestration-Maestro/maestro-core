//! Small read-only ports; authority and transport startup are deliberately absent.
use crate::{
    policy::{
        acquisition::AcquisitionProfile,
        decisions::{Decisions, Promotion},
        identity::IdentityMigration,
        schema::SourcePolicy,
    },
    refusal::Refusal,
};
use maestro_kernel::{artifact::Digest, scope::ScopeSet};
use maestro_knowledge::collection::Declaration;
use maestro_knowledge::collection::PolicyReference as Ref;
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
/// A read-only digest-bound policy; obtaining it creates no authority or effects.
#[derive(Debug)]
pub struct CheckedPolicy {
    /// Exact original immutable baseline reference.
    pub(crate) reference: Ref,
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
