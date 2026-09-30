//! Strict version-one source-policy wire contracts.

use super::resource::Resource;
use super::shape;
use maestro_kernel::artifact::Digest;
use maestro_knowledge::collection::PolicyReference as Ref;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

/// The closed set of `AutomaticClass` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutomaticClass {
    /// `selected_profiles`.
    #[serde(rename = "selected_profiles")]
    SelectedProfiles,
    /// `cleanup`.
    #[serde(rename = "cleanup")]
    Cleanup,
    /// `s1_chunk_strategy`.
    #[serde(rename = "s1_chunk_strategy")]
    S1ChunkStrategy,
    /// `dedup_keys`.
    #[serde(rename = "dedup_keys")]
    DedupKeys,
    /// `new_knowledge_exclusions`.
    #[serde(rename = "new_knowledge_exclusions")]
    NewKnowledgeExclusions,
}

/// The closed set of `BaselineKind` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BaselineKind {
    /// `local`.
    #[serde(rename = "local")]
    Local,
    /// `catalog`.
    #[serde(rename = "catalog")]
    Catalog,
}

/// `AdaptationPolicy`: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdaptationPolicy {
    /// Declared `matrix`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub matrix: Ref,
    /// Declared `thresholds`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub thresholds: Ref,
    /// Declared `baseline`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub baseline: Ref,
    /// Declared `minimum_sample`; no inferred default.
    pub minimum_sample: NonZeroU64,
    /// Declared `consecutive_runs`; no inferred default.
    pub consecutive_runs: NonZeroU64,
    /// Declared `activation_interval_ms`; no inferred default.
    pub activation_interval_ms: NonZeroU64,
    /// Declared `automatic_classes`; no inferred default.
    #[serde(deserialize_with = "shape::names")]
    #[schemars(length(max = 10000))]
    pub automatic_classes: Vec<AutomaticClass>,
}

/// `AcquisitionManifest`: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AcquisitionManifest {
    /// Declared `resource`; no inferred default.
    #[serde(flatten)]
    pub resource: Resource<ManifestSchema>,
    /// Declared `baseline`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub baseline: Ref,
    /// Declared `baseline_kind`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub baseline_kind: BaselineKind,
    /// Declared `proposals`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(max = 10000))]
    pub proposals: Vec<Ref>,
    /// Declared `activations`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(max = 10000))]
    pub activations: Vec<Ref>,
    /// Declared `active`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub active: Ref,
    /// Declared `effective_digest`; no inferred default.
    #[schemars(with = "String")]
    pub effective_digest: Digest,
}

/// The executable `AcquisitionManifest` version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum ManifestSchema {
    /// Version one only.
    #[serde(rename = "maestro-acquisition-manifest/1")]
    V1,
}

impl AutomaticClass {
    /// Map the plan's six Change variants to its five automatic classes.
    /// Unknown change names hold rather than becoming an automatic permission.
    #[must_use]
    pub fn for_change(change: &str) -> Option<Self> {
        match change {
            "select_profile" => Some(Self::SelectedProfiles),
            "set_cleanup" => Some(Self::Cleanup),
            "set_s1_chunk_strategy" => Some(Self::S1ChunkStrategy),
            "set_dedup_keys" => Some(Self::DedupKeys),
            "add_knowledge_exclusion" | "add_asset_only" => Some(Self::NewKnowledgeExclusions),
            _ => None,
        }
    }
}
