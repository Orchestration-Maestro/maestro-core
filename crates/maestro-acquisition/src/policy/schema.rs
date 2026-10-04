//! Strict version-one source-policy wire contracts.

use super::shape;
use maestro_knowledge::collection::PolicyReference as Ref;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::resource::Resource;
/// `SourcePolicy`: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourcePolicy {
    /// Declared `resource`; no inferred default.
    #[serde(flatten)]
    pub resource: Resource<PolicySchema>,
    /// Declared `sources`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(min = 1, max = 1000))]
    pub sources: Vec<Source>,
    /// Declared `registries`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(max = 10000))]
    pub registries: Vec<Ref>,
    /// Declared `profiles`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub profiles: Ref,
    /// Declared `acquisition_profiles`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(min = 1, max = 1000))]
    pub acquisition_profiles: Vec<Ref>,
    /// Declared `address_table`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub address_table: Ref,
    /// Declared `aggregate_limits`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub aggregate_limits: Limits,
    /// Declared `adaptation`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub adaptation: AdaptationPolicy,
    /// Declared `retention_rule`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub retention_rule: Ref,
    /// Declared `qualification`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub qualification: Ref,
}
use super::{limits::Limits, manifest::AdaptationPolicy, source::Source};

/// The executable source-policy version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum PolicySchema {
    /// Version one only.
    #[serde(rename = "maestro-source-policy/1")]
    V1,
}
