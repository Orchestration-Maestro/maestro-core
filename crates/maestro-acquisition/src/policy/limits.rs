//! Strict version-one source-policy wire contracts.

use super::shape;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

/// The closed set of `XmlEntities` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum XmlEntities {
    /// `disabled`.
    #[serde(rename = "disabled")]
    Disabled,
}

/// `DecodeLimits`: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecodeLimits {
    /// Declared `expanded_bytes`; no inferred default.
    pub expanded_bytes: NonZeroU64,
    /// Declared `expansion_ratio`; no inferred default.
    pub expansion_ratio: NonZeroU64,
    /// Declared `nested_levels`; no inferred default.
    pub nested_levels: NonZeroU64,
    /// Declared `members`; no inferred default.
    pub members: NonZeroU64,
    /// Declared `decoded_pixels`; no inferred default.
    pub decoded_pixels: NonZeroU64,
    /// Declared `elapsed_ms`; no inferred default.
    pub elapsed_ms: NonZeroU64,
    /// Declared `memory_bytes`; no inferred default.
    pub memory_bytes: NonZeroU64,
    /// Declared `xml_entities`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub xml_entities: XmlEntities,
}

/// Limits: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// Declared `requests`; no inferred default.
    pub requests: NonZeroU64,
    /// Declared `pages`; no inferred default.
    pub pages: NonZeroU64,
    /// Declared `partitions`; no inferred default.
    pub partitions: NonZeroU64,
    /// Declared `redirects`; no inferred default.
    pub redirects: NonZeroU64,
    /// Declared `depth`; no inferred default.
    pub depth: NonZeroU64,
    /// Declared `elapsed_ms`; no inferred default.
    pub elapsed_ms: NonZeroU64,
    /// Declared `wire_bytes`; no inferred default.
    pub wire_bytes: NonZeroU64,
    /// Declared `dom_bytes`; no inferred default.
    pub dom_bytes: NonZeroU64,
    /// Declared `asset_bytes`; no inferred default.
    pub asset_bytes: NonZeroU64,
    /// Declared `staging_bytes`; no inferred default.
    pub staging_bytes: NonZeroU64,
    /// Declared `cpu_millicores`; no inferred default.
    pub cpu_millicores: NonZeroU64,
    /// Declared `memory_bytes`; no inferred default.
    pub memory_bytes: NonZeroU64,
    /// Declared `source_runs`; no inferred default.
    pub source_runs: NonZeroU64,
    /// Declared `origin_concurrency`; no inferred default.
    pub origin_concurrency: NonZeroU64,
    /// Declared `free_reserve_bytes`; no inferred default.
    pub free_reserve_bytes: NonZeroU64,
    /// Declared `gpu_reserve_bytes`; no inferred default.
    pub gpu_reserve_bytes: NonZeroU64,
    /// Declared `origin_interval_ms`; no inferred default.
    pub origin_interval_ms: NonZeroU64,
    /// Declared `retries`; no inferred default.
    pub retries: u64,
    /// Declared `max_backoff_ms`; no inferred default.
    pub max_backoff_ms: u64,
    /// Declared `gpu_batches`; no inferred default.
    pub gpu_batches: u64,
    /// Declared `gpu_bytes`; no inferred default.
    pub gpu_bytes: u64,
    /// Declared `decode`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub decode: DecodeLimits,
}
