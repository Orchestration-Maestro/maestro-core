//! Shared scoped resource identity; never an acquisition grant.
use super::shape;
use maestro_knowledge::collection::PolicyReference as Ref;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;
/// The closed set of Visibility values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// `public`.
    #[serde(rename = "public")]
    Public,
    /// `private`.
    #[serde(rename = "private")]
    Private,
}

/// Resource: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[serde(bound(deserialize = "S: Deserialize<'de>"))]
pub struct Resource<S> {
    /// Declared `schema`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub schema: S,
    /// Declared `id`; no inferred default.
    #[serde(deserialize_with = "shape::id")]
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$")
    )]
    pub id: String,
    /// Declared `version`; no inferred default.
    pub version: NonZeroU64,
    /// Declared `collection_id`; no inferred default.
    #[serde(deserialize_with = "shape::id")]
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$")
    )]
    pub collection_id: String,
    /// Declared `visibility`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub visibility: Visibility,
    /// Declared `scope_tags`; no inferred default.
    #[serde(deserialize_with = "shape::scopes")]
    #[schemars(length(max = 10000))]
    pub scope_tags: Vec<String>,
    /// Declared `owner_ref`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub owner_ref: Ref,
}
