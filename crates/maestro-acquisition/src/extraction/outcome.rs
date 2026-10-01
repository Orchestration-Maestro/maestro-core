//! Immutable checked handles and content-free registry outcomes.
use super::{
    detect::SafePartial,
    model::{ProfileDefinition, Registry},
};
use crate::refusal::Refusal;
use maestro_knowledge::collection::PolicyReference as Ref;

/// Content-free failures; none selects an implicit default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryUnavailable {
    /// Explicitly disabled by composition.
    Disabled,
    /// Required resource is absent.
    Missing,
    /// Invalid schema, substituted bytes or inconsistent identity.
    Corrupt,
    /// Separate current-platform reviewed qualification is absent.
    Unqualified,
    /// Current principal cannot read the collection scopes.
    Access,
}
impl From<Refusal> for RegistryUnavailable {
    fn from(value: Refusal) -> Self {
        match value {
            Refusal::Missing => Self::Missing,
            Refusal::Unqualified => Self::Unqualified,
            Refusal::Access => Self::Access,
            _ => Self::Corrupt,
        }
    }
}

/// Explicit safe held disposition, never accepted/searchable content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeldReason {
    /// Unsupported, incomplete, unsafe or unqualified evidence.
    Unknown,
    /// More than one content-backed profile matches.
    Ambiguous,
}

/// Digest-bound outcome with evidence and safe retained partial data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileSelection {
    /// One independently admitted content-backed definition matched.
    Selected {
        /// Exact approved profile identity/digest.
        profile: Ref,
        /// Exact detection/structure receipt references.
        evidence: Vec<Ref>,
    },
    /// Explicit safe retention, not arbitrary trusted fallback.
    Held {
        /// Accountable uncertainty or contradiction.
        reason: HeldReason,
        /// Explicit registry-owned retention profile.
        safe_profile: Ref,
        /// Exact detection/structure receipt references.
        evidence: Vec<Ref>,
        /// Bounded obtainable original text, metadata and assets.
        partial: SafePartial,
    },
}

/// Immutable core-checked registry; external adapters cannot manufacture fields.
#[derive(Debug)]
pub struct CheckedRegistry {
    /// Exact immutable registry reference.
    pub(crate) reference: Ref,
    /// Strict parsed registry resource.
    pub(crate) registry: Registry,
    /// Current principal/platform used for resolution.
    pub(crate) principal: (String, String),
    /// Definitions bound to separate reviewed original bytes.
    pub(crate) qualified: Vec<Ref>,
    /// Policy-eligible refs, set only by the core policy binding wrapper.
    pub(crate) eligible: Vec<Ref>,
}
impl CheckedRegistry {
    /// Exact registry identity; no mutable definition access.
    #[must_use]
    pub fn reference(&self) -> &Ref {
        &self.reference
    }
    /// Immutable definition for downstream protected-field comparison.
    #[must_use]
    pub fn profile(&self, reference: &Ref) -> Option<&ProfileDefinition> {
        self.registry
            .profiles
            .iter()
            .find(|profile| profile.reference() == *reference)
            .map(|profile| &profile.definition)
    }
}
