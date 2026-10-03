//! Strict version-one source-policy wire contracts.

use super::{resource::Resource, source::Selector};
use super::{shape, shape::RequiredNullable};
use maestro_knowledge::collection::PolicyReference as Ref;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The closed set of Action values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// `deny_fetch`.
    #[serde(rename = "deny_fetch")]
    DenyFetch,
    /// `exclude_from_knowledge`.
    #[serde(rename = "exclude_from_knowledge")]
    ExcludeFromKnowledge,
    /// `asset_only`.
    #[serde(rename = "asset_only")]
    AssetOnly,
}

/// The closed set of `PromotionAction` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PromotionAction {
    /// `promote_knowledge`.
    #[serde(rename = "promote_knowledge")]
    PromoteKnowledge,
}

/// Decision: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    /// Declared `id`; no inferred default.
    #[serde(deserialize_with = "shape::id")]
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$")
    )]
    pub id: String,
    /// Declared `selector`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub selector: Selector,
    /// Declared `action`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub action: Action,
    /// Declared `reason_code`; no inferred default.
    #[serde(deserialize_with = "shape::id")]
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$")
    )]
    pub reason_code: String,
    /// Declared `explanation`; no inferred default.
    #[serde(deserialize_with = "shape::text")]
    #[schemars(length(max = 4096))]
    pub explanation: String,
    /// Declared `authority`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub authority: Ref,
    /// Declared `evidence`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(max = 10000))]
    pub evidence: Vec<Ref>,
    /// Declared `effective_at`; no inferred default.
    #[serde(deserialize_with = "shape::time")]
    pub effective_at: String,
    /// Declared `review_at`; no inferred default.
    #[serde(deserialize_with = "shape::time")]
    pub review_at: String,
    /// Declared `expires_at`; no inferred default.
    #[serde(deserialize_with = "shape::nullable_time")]
    #[schemars(with = "RequiredNullable<String>")]
    pub expires_at: Option<String>,
    /// Declared `reversal`; no inferred default.
    #[serde(deserialize_with = "shape::nullable_object")]
    #[schemars(with = "RequiredNullable<Ref>")]
    pub reversal: Option<Ref>,
}

/// Promotion: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Promotion {
    /// Declared `id`; no inferred default.
    #[serde(deserialize_with = "shape::id")]
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$")
    )]
    pub id: String,
    /// Declared `selector`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub selector: Selector,
    /// Declared `action`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub action: PromotionAction,
    /// Declared `reason_code`; no inferred default.
    #[serde(deserialize_with = "shape::id")]
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$")
    )]
    pub reason_code: String,
    /// Declared `explanation`; no inferred default.
    #[serde(deserialize_with = "shape::text")]
    #[schemars(length(max = 4096))]
    pub explanation: String,
    /// Declared `authority`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub authority: Ref,
    /// Declared `evidence`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(max = 10000))]
    pub evidence: Vec<Ref>,
    /// Declared `effective_at`; no inferred default.
    #[serde(deserialize_with = "shape::time")]
    pub effective_at: String,
    /// Declared `review_at`; no inferred default.
    #[serde(deserialize_with = "shape::time")]
    pub review_at: String,
    /// Declared `expires_at`; no inferred default.
    #[serde(deserialize_with = "shape::nullable_time")]
    #[schemars(with = "RequiredNullable<String>")]
    pub expires_at: Option<String>,
    /// Declared `reversal`; no inferred default.
    #[serde(deserialize_with = "shape::nullable_object")]
    #[schemars(with = "RequiredNullable<Ref>")]
    pub reversal: Option<Ref>,
}

/// Decisions: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Decisions {
    /// Declared `resource`; no inferred default.
    #[serde(flatten)]
    pub resource: Resource<DecisionSchema>,
    /// Declared `entries`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(max = 10000))]
    pub entries: Vec<Decision>,
    /// Declared `qualification`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub qualification: Ref,
}

/// Promotions: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Promotions {
    /// Declared `resource`; no inferred default.
    #[serde(flatten)]
    pub resource: Resource<PromotionSchema>,
    /// Declared `entries`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(max = 10000))]
    pub entries: Vec<Promotion>,
    /// Declared `qualification`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub qualification: Ref,
}

/// The executable Decisions version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum DecisionSchema {
    /// Version one only.
    #[serde(rename = "maestro-source-decisions/1")]
    V1,
}

/// The executable Promotions version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum PromotionSchema {
    /// Version one only.
    #[serde(rename = "maestro-source-promotions/1")]
    V1,
}
