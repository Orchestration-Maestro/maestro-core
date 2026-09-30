//! Strict version-one source-policy wire contracts.

use super::limits::Limits;
use super::{shape, shape::RequiredNullable};
use maestro_knowledge::collection::PolicyReference as Ref;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::num::{NonZeroU16, NonZeroU64};

/// The closed set of Scheme values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Scheme {
    /// `https`.
    #[serde(rename = "https")]
    Https,
}

/// The closed set of Purpose values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    /// `content`.
    #[serde(rename = "content")]
    Content,
    /// `authentication`.
    #[serde(rename = "authentication")]
    Authentication,
    /// `asset`.
    #[serde(rename = "asset")]
    Asset,
}

/// The closed set of `QueryOrder` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryOrder {
    /// `preserve`.
    #[serde(rename = "preserve")]
    Preserve,
    /// `sort`.
    #[serde(rename = "sort")]
    Sort,
}

/// The closed set of `RepeatedQueries` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RepeatedQueries {
    /// `preserve`.
    #[serde(rename = "preserve")]
    Preserve,
    /// `reject`.
    #[serde(rename = "reject")]
    Reject,
}

/// The closed set of Fragment values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Fragment {
    /// `discard_for_fetch`.
    #[serde(rename = "discard_for_fetch")]
    DiscardForFetch,
}

/// The closed set of `SyncMode` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    /// `manual`.
    #[serde(rename = "manual")]
    Manual,
    /// `one_off`.
    #[serde(rename = "one_off")]
    OneOff,
    /// `watch`.
    #[serde(rename = "watch")]
    Watch,
}

/// Origin: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    /// Declared `id`; no inferred default.
    #[serde(deserialize_with = "shape::id")]
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$")
    )]
    pub id: String,
    /// Declared `scheme`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub scheme: Scheme,
    /// Declared `host`; no inferred default.
    #[serde(deserialize_with = "shape::host")]
    pub host: String,
    /// Declared `port`; no inferred default.
    pub port: NonZeroU16,
    /// Declared `path_prefixes`; no inferred default.
    #[serde(deserialize_with = "shape::paths")]
    #[schemars(length(max = 10000))]
    pub path_prefixes: Vec<String>,
    /// Declared `purpose`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub purpose: Purpose,
    /// Declared `private_grant`; no inferred default.
    #[serde(deserialize_with = "shape::nullable_id")]
    #[schemars(with = "RequiredNullable<String>")]
    pub private_grant: Option<String>,
}

/// Declarative Discovery; unknown predicates refuse.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Discovery {
    /// `links`.
    #[serde(rename = "links")]
    Links {
        /// Explicit `depth`.
        depth: NonZeroU64,
    },
    /// `sitemap`.
    #[serde(rename = "sitemap")]
    Sitemap {
        /// Explicit `url`.
        #[serde(deserialize_with = "shape::url")]
        url: String,
    },
    /// `api`.
    #[serde(rename = "api")]
    Api {
        /// Explicit `mapping`.
        #[serde(deserialize_with = "shape::object")]
        mapping: Ref,
    },
    /// `repository`.
    #[serde(rename = "repository")]
    Repository {
        /// Explicit `owner`.
        #[serde(deserialize_with = "shape::id")]
        owner: String,
        /// Explicit `repository`.
        #[serde(deserialize_with = "shape::id")]
        repository: String,
        /// Explicit `r#ref`.
        #[serde(deserialize_with = "shape::text")]
        r#ref: String,
        /// Explicit `paths`.
        #[serde(deserialize_with = "shape::texts")]
        paths: Vec<String>,
        /// Explicit `submodules`.
        #[serde(deserialize_with = "shape::texts")]
        submodules: Vec<String>,
        /// Explicit `large_files`.
        #[serde(deserialize_with = "shape::texts")]
        large_files: Vec<String>,
    },
}

/// Selector: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Selector {
    /// Declared `source_id`; no inferred default.
    #[serde(deserialize_with = "shape::id")]
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$")
    )]
    pub source_id: String,
    /// Declared `origin`; no inferred default.
    #[serde(deserialize_with = "shape::nullable_id")]
    #[schemars(with = "RequiredNullable<String>")]
    pub origin: Option<String>,
    /// Declared `path_prefix`; no inferred default.
    #[serde(deserialize_with = "shape::nullable_path")]
    #[schemars(with = "RequiredNullable<String>")]
    pub path_prefix: Option<String>,
    /// Declared `object_ids`; no inferred default.
    #[serde(deserialize_with = "shape::ids")]
    #[schemars(length(max = 10000))]
    pub object_ids: Vec<String>,
    /// Declared `versions`; no inferred default.
    #[serde(deserialize_with = "shape::texts")]
    #[schemars(length(max = 10000))]
    pub versions: Vec<String>,
    /// Declared `channels`; no inferred default.
    #[serde(deserialize_with = "shape::ids")]
    #[schemars(length(max = 10000))]
    pub channels: Vec<String>,
    /// Declared `media_types`; no inferred default.
    #[serde(deserialize_with = "shape::texts")]
    #[schemars(length(max = 10000))]
    pub media_types: Vec<String>,
}

/// Robots: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Robots {
    /// Declared `agent`; no inferred default.
    #[serde(deserialize_with = "shape::text")]
    #[schemars(length(max = 4096))]
    pub agent: String,
    /// Declared `r#override`; no inferred default.
    #[serde(deserialize_with = "shape::nullable_object")]
    #[schemars(with = "RequiredNullable<Ref>")]
    pub r#override: Option<Ref>,
    /// Declared `rules_max_bytes`; no inferred default.
    pub rules_max_bytes: NonZeroU64,
    /// Declared `cache_ttl_ms`; no inferred default.
    pub cache_ttl_ms: NonZeroU64,
}

/// `IdentityRule`: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdentityRule {
    /// Declared `version`; no inferred default.
    pub version: NonZeroU64,
    /// Declared `meaningful_queries`; no inferred default.
    #[serde(deserialize_with = "shape::texts")]
    #[schemars(length(max = 10000))]
    pub meaningful_queries: Vec<String>,
    /// Declared `ignored_tracking_queries`; no inferred default.
    #[serde(deserialize_with = "shape::texts")]
    #[schemars(length(max = 10000))]
    pub ignored_tracking_queries: Vec<String>,
    /// Declared `query_order`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub query_order: QueryOrder,
    /// Declared `repeated_queries`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub repeated_queries: RepeatedQueries,
    /// Declared `fragment`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub fragment: Fragment,
    /// Declared `migration`; no inferred default.
    #[serde(deserialize_with = "shape::nullable_object")]
    #[schemars(with = "RequiredNullable<Ref>")]
    pub migration: Option<Ref>,
}

/// `SyncPolicy`: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SyncPolicy {
    /// Declared `mode`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub mode: SyncMode,
    /// Declared `timer_period_ms`; no inferred default.
    #[serde(deserialize_with = "shape::nullable")]
    #[schemars(with = "RequiredNullable<NonZeroU64>")]
    pub timer_period_ms: Option<NonZeroU64>,
    /// Declared `overlap_ms`; no inferred default.
    pub overlap_ms: u64,
    /// Declared `clock_skew_ms`; no inferred default.
    pub clock_skew_ms: u64,
    /// Declared `revision_fields`; no inferred default.
    #[serde(deserialize_with = "shape::texts")]
    #[schemars(length(max = 10000))]
    pub revision_fields: Vec<String>,
}

/// Source: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Declared `id`; no inferred default.
    #[serde(deserialize_with = "shape::id")]
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$")
    )]
    pub id: String,
    /// Declared `origins`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(max = 10000))]
    pub origins: Vec<Origin>,
    /// Declared `seeds`; no inferred default.
    #[serde(deserialize_with = "shape::urls")]
    #[schemars(length(max = 10000))]
    pub seeds: Vec<String>,
    /// Declared `discovery`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(max = 10000))]
    pub discovery: Vec<Discovery>,
    /// Declared `selectors`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(max = 10000))]
    pub selectors: Vec<Selector>,
    /// Declared `decisions`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(max = 10000))]
    pub decisions: Vec<Ref>,
    /// Declared `promotions`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(max = 10000))]
    pub promotions: Vec<Ref>,
    /// Declared `robots`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub robots: Robots,
    /// Declared `limits`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub limits: Limits,
    /// Declared `identity`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub identity: IdentityRule,
    /// Declared `auth_role`; no inferred default.
    #[serde(deserialize_with = "shape::nullable_id")]
    #[schemars(with = "RequiredNullable<String>")]
    pub auth_role: Option<String>,
    /// Declared `connector`; no inferred default.
    #[serde(deserialize_with = "shape::nullable_object")]
    #[schemars(with = "RequiredNullable<Ref>")]
    pub connector: Option<Ref>,
    /// Declared `wiki_mapping`; no inferred default.
    #[serde(deserialize_with = "shape::nullable_object")]
    #[schemars(with = "RequiredNullable<Ref>")]
    pub wiki_mapping: Option<Ref>,
    /// Declared `acquisition_profile`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub acquisition_profile: Ref,
    /// Declared `selected_profiles`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(max = 10000))]
    pub selected_profiles: Vec<Ref>,
    /// Declared `sync`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub sync: SyncPolicy,
}
