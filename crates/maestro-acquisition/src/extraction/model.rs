//! Strict immutable profile definitions; field order is part of digest version two.
use super::detect::{Detector, Structure};
use crate::policy::{limits::DecodeLimits, resource::Resource};
use maestro_kernel::artifact::Digest;
use maestro_knowledge::{
    collection::PolicyReference as Ref,
    strict_json::{name, object, objects},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

/// Exact registry schema, never inferred.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum RegistrySchema {
    /// Version two.
    #[serde(rename = "maestro-extraction-registry/2")]
    V2,
}

/// Immutable manifest-bound registry.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// Collection ownership, visibility and scopes.
    #[serde(flatten)]
    pub resource: Resource<RegistrySchema>,
    /// Explicit definitions, including the safe unknown definition.
    pub profiles: Vec<Profile>,
    /// Explicit safe retained-content profile; not searchable acceptance.
    #[serde(deserialize_with = "object")]
    pub unknown_profile: Ref,
    /// Exact external registry qualification.
    #[serde(deserialize_with = "object")]
    pub qualification: Ref,
}

/// Separately approved definition digest and its strict typed preimage.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// SHA-256 of the version-prefixed typed definition bytes.
    #[schemars(with = "String", regex(pattern = "^[0-9a-f]{64}$"))]
    pub definition_digest: Digest,
    /// All fields other than the digest, in stable serialized order.
    #[serde(flatten)]
    pub definition: ProfileDefinition,
}

/// The definition preimage. Reordering fields requires a digest-version bump.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileDefinition {
    /// Logical profile identity.
    pub id: String,
    /// Positive immutable definition version.
    pub version: NonZeroU64,
    /// Content evidence predicates; every predicate must match.
    pub detectors: Vec<Detector>,
    /// Required bounded structural predicates.
    pub structures: Vec<Structure>,
    /// Declared supported language IDs, not inferred permission.
    pub languages: Vec<String>,
    /// Pinned installed extractor; never a download instruction.
    #[serde(deserialize_with = "object")]
    pub extractor: Ref,
    /// Approved processing identities.
    #[serde(deserialize_with = "object")]
    pub processing: Processing,
    /// Protected finite cumulative decoder ceilings.
    #[serde(deserialize_with = "object")]
    pub decode_limits: DecodeLimits,
    /// Exact typed output schema.
    #[serde(deserialize_with = "object")]
    pub output_schema: Ref,
    /// Protected source-correspondence requirements.
    pub required_fidelity: Vec<String>,
    /// Protected admission and permission rule IDs.
    pub admission_rules: Vec<String>,
    /// Exact runtime/model/plugin artifacts.
    #[serde(deserialize_with = "objects")]
    pub artifacts: Vec<Ref>,
    /// Explicit qualified host platforms.
    pub platforms: Vec<String>,
    /// Definition state cannot self-grant external qualification.
    #[serde(deserialize_with = "name")]
    pub qualification_state: QualificationState,
    /// Exact qualification closure, including gold and approved thresholds.
    #[serde(deserialize_with = "objects")]
    pub qualification_evidence: Vec<Ref>,
}

/// Approved processing identities, not executable snippets.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Processing {
    /// Immutable whole cleanup rule set.
    #[serde(deserialize_with = "object")]
    pub cleanup: Ref,
    /// Qualified immutable S1 strategy.
    #[serde(deserialize_with = "object")]
    pub chunk: Ref,
    /// Immutable whole deduplication key set.
    #[serde(deserialize_with = "object")]
    pub dedup: Ref,
}

/// Proposal state never substitutes for external reviewed evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QualificationState {
    /// Proposed only.
    Proposed,
    /// Still requires exact external qualification evidence.
    Qualified,
    /// Incomplete or failed.
    Held,
    /// No longer eligible.
    Revoked,
}
