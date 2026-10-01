//! Strict version-one source-policy wire contracts.

use super::resource::Resource;
use super::{shape, shape::RequiredNullable};
use maestro_knowledge::collection::PolicyReference as Ref;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

pub use maestro_kernel::acquisition::Transport;

/// The closed set of `DocumentState` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentState {
    /// `dom_content_loaded`.
    #[serde(rename = "dom_content_loaded")]
    DomContentLoaded,
    /// `load`.
    #[serde(rename = "load")]
    Load,
}

/// Attribute: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Attribute {
    /// Declared `name`; no inferred default.
    #[serde(deserialize_with = "shape::id")]
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$")
    )]
    pub name: String,
    /// Declared `value`; no inferred default.
    #[serde(deserialize_with = "shape::text")]
    #[schemars(length(max = 4096))]
    pub value: String,
}

/// `DomStep`: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomStep {
    /// Declared `tag`; no inferred default.
    #[serde(deserialize_with = "shape::id")]
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$")
    )]
    pub tag: String,
    /// Declared `attributes`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(max = 8))]
    pub attributes: Vec<Attribute>,
}

/// Bounded direct-child DOM path, never CSS or script.
pub type DomPath = Vec<DomStep>;

/// Declarative `ReadyCondition`; unknown predicates refuse.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReadyCondition {
    /// `element_present`.
    #[serde(rename = "element_present")]
    ElementPresent {
        /// Explicit `selector`.
        #[serde(deserialize_with = "shape::objects")]
        #[schemars(length(min = 1, max = 32))]
        selector: DomPath,
    },
    /// `element_text`.
    #[serde(rename = "element_text")]
    ElementText {
        /// Explicit `selector`.
        #[serde(deserialize_with = "shape::objects")]
        #[schemars(length(min = 1, max = 32))]
        selector: DomPath,
        /// Explicit `contains`.
        #[serde(deserialize_with = "shape::text")]
        contains: String,
    },
    /// `element_absent`.
    #[serde(rename = "element_absent")]
    ElementAbsent {
        /// Explicit `selector`.
        #[serde(deserialize_with = "shape::objects")]
        #[schemars(length(min = 1, max = 32))]
        selector: DomPath,
    },
}

/// Readiness: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Readiness {
    /// Declared `document_state`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub document_state: DocumentState,
    /// Declared `all_of`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(min = 1, max = 32))]
    pub all_of: Vec<ReadyCondition>,
    /// Declared `timeout_ms`; no inferred default.
    pub timeout_ms: NonZeroU64,
    /// Declared `poll_interval_ms`; no inferred default.
    pub poll_interval_ms: NonZeroU64,
    /// Declared `stable_for_ms`; no inferred default.
    pub stable_for_ms: NonZeroU64,
}

/// `AcquisitionProfile`: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AcquisitionProfile {
    /// Declared `resource`; no inferred default.
    #[serde(flatten)]
    pub resource: Resource<AcquisitionSchema>,
    /// Declared `transport`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub transport: Transport,
    /// Declared `adapter`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub adapter: Ref,
    /// Declared `required_capabilities`; no inferred default.
    #[serde(deserialize_with = "shape::ids")]
    #[schemars(length(max = 10000))]
    pub required_capabilities: Vec<String>,
    /// Declared `readiness`; no inferred default.
    #[serde(deserialize_with = "shape::nullable_object")]
    #[schemars(with = "RequiredNullable<Readiness>")]
    pub readiness: Option<Readiness>,
    /// Declared `qualification`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub qualification: Ref,
}

/// The executable `AcquisitionProfile` version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum AcquisitionSchema {
    /// Version one only.
    #[serde(rename = "maestro-acquisition-profile/1")]
    V1,
}
