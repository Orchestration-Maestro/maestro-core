//! Strict version-one source-policy wire contracts.

use super::resource::Resource;
use super::{shape, shape::RequiredNullable};
use maestro_knowledge::collection::PolicyReference as Ref;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Declarative `FieldStep`; unknown predicates refuse.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FieldStep {
    /// `field`.
    #[serde(rename = "field")]
    Field {
        /// Explicit `name`.
        #[serde(deserialize_with = "shape::text")]
        name: String,
    },
    /// `index`.
    #[serde(rename = "index")]
    Index {
        /// Explicit `value`.
        value: u32,
    },
}

/// At most 32 literal field/index steps.
pub type FieldPath = Vec<FieldStep>;

/// Declarative Pagination; unknown predicates refuse.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Pagination {
    /// `cursor`.
    #[serde(rename = "cursor")]
    Cursor {
        /// Explicit `request_field`.
        #[serde(deserialize_with = "shape::text")]
        request_field: String,
        /// Explicit `response_path`.
        #[serde(deserialize_with = "shape::objects")]
        response_path: FieldPath,
        /// Explicit `terminal_path`.
        #[serde(deserialize_with = "shape::objects")]
        terminal_path: FieldPath,
    },
    /// `next_link`.
    #[serde(rename = "next_link")]
    NextLink {
        /// Explicit `response_path`.
        #[serde(deserialize_with = "shape::objects")]
        response_path: FieldPath,
        /// Explicit `terminal_path`.
        #[serde(deserialize_with = "shape::objects")]
        terminal_path: FieldPath,
    },
}

/// The closed set of `ContentKind` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    /// `html`.
    #[serde(rename = "html")]
    Html,
    /// `markdown`.
    #[serde(rename = "markdown")]
    Markdown,
    /// `blocks`.
    #[serde(rename = "blocks")]
    Blocks,
}

/// The closed set of `PermissionSemantics` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PermissionSemantics {
    /// `explicit_scopes`.
    #[serde(rename = "explicit_scopes")]
    ExplicitScopes,
    /// `inherit_with_restrictions`.
    #[serde(rename = "inherit_with_restrictions")]
    InheritWithRestrictions,
}

/// The closed set of Withdrawal values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Withdrawal {
    /// `explicit_tombstone`.
    #[serde(rename = "explicit_tombstone")]
    ExplicitTombstone,
    /// `complete_inventory`.
    #[serde(rename = "complete_inventory")]
    CompleteInventory,
}

/// `WikiMapping`: the plan’s strict wire record.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiMapping {
    /// Declared `resource`; no inferred default.
    #[serde(flatten)]
    pub resource: Resource<WikiSchema>,
    /// Declared `origin_id`; no inferred default.
    #[serde(deserialize_with = "shape::id")]
    #[schemars(
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$")
    )]
    pub origin_id: String,
    /// Declared `list_endpoint`; no inferred default.
    #[serde(deserialize_with = "shape::path")]
    pub list_endpoint: String,
    /// Declared `item_endpoint`; no inferred default.
    #[serde(deserialize_with = "shape::path")]
    pub item_endpoint: String,
    /// Declared `items_path`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    pub items_path: FieldPath,
    /// Declared `identity_path`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    pub identity_path: FieldPath,
    /// Declared `parent_path`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    pub parent_path: FieldPath,
    /// Declared `revision_path`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    pub revision_path: FieldPath,
    /// Declared `content_path`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    pub content_path: FieldPath,
    /// Declared `content_kind`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub content_kind: ContentKind,
    /// Declared `block_mapping`; no inferred default.
    #[serde(deserialize_with = "shape::nullable_object")]
    #[schemars(with = "RequiredNullable<Ref>")]
    pub block_mapping: Option<Ref>,
    /// Declared `attachments_path`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    pub attachments_path: FieldPath,
    /// Declared `permissions_path`; no inferred default.
    #[serde(deserialize_with = "shape::objects")]
    pub permissions_path: FieldPath,
    /// Declared `permission_semantics`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub permission_semantics: PermissionSemantics,
    /// Declared `pagination`; no inferred default.
    #[serde(deserialize_with = "shape::object")]
    pub pagination: Pagination,
    /// Declared `withdrawal`; no inferred default.
    #[serde(deserialize_with = "shape::name")]
    pub withdrawal: Withdrawal,
    /// Declared `tombstone_path`; no inferred default.
    #[serde(deserialize_with = "shape::nullable")]
    #[schemars(with = "RequiredNullable<FieldPath>")]
    pub tombstone_path: Option<FieldPath>,
}

/// The executable `WikiMapping` version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum WikiSchema {
    /// Version one only.
    #[serde(rename = "maestro-wiki-mapping/1")]
    V1,
}
