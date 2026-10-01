//! Immutable processing data; engines and quality qualification remain S1-owned.
use crate::{
    Ref, Refusal,
    extraction::detect::Structure,
    policy::{resource::Resource, shape},
};
use maestro_canonicalization::ChunkProfile;
use maestro_kernel::gateway::{Limits, ModelCard, Role, card_v2::Observation};
use maestro_knowledge::strict_json::{name, names, object, objects};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::{collections::BTreeSet, num::NonZeroU64};

/// Closed cleanup artifact version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CleanupSchema {
    /// Version one.
    #[serde(rename = "maestro-cleanup-rules/1")]
    V1,
}
/// Closed chunk artifact version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ChunkSchema {
    /// Version one.
    #[serde(rename = "maestro-s1-chunk-strategy/1")]
    V1,
}
/// Closed dedup artifact version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum DedupSchema {
    /// Version one.
    #[serde(rename = "maestro-dedup-keys/1")]
    V1,
}
/// Only navigation omission is available; no rewrite operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CleanupAction {
    /// Omit one matched navigation unit, retaining original evidence.
    OmitNavigation,
}
/// One ordered, bounded cleanup rule over N15 observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CleanupRule {
    /// Unique rule identity.
    #[serde(deserialize_with = "shape::id")]
    #[schemars(length(max = 128), regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$"))]
    pub id: String,
    /// The existing observation AST, not a new selector language.
    #[serde(deserialize_with = "object")]
    pub selector: Structure,
    /// Closed action.
    #[serde(deserialize_with = "name")]
    pub action: CleanupAction,
    /// Auditable reason identity.
    #[serde(deserialize_with = "shape::id")]
    #[schemars(length(max = 128), regex(pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]*$"))]
    pub reason_code: String,
}
/// Ordered whole cleanup set. An empty set explicitly does no cleanup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CleanupRules {
    /// Scoped logical metadata, serialized first.
    #[serde(flatten)]
    pub resource: Resource<CleanupSchema>,
    /// Unique rules sharing N15's cumulative AST bounds.
    #[serde(deserialize_with = "rules")]
    #[schemars(length(max = 1000))]
    pub rules: Vec<CleanupRule>,
    /// External admission, never self-approval.
    #[serde(deserialize_with = "object")]
    pub qualification: Ref,
}
/// Validate the entire rule set before any matching.
fn rules<'de, D: Deserializer<'de>>(decoder: D) -> Result<Vec<CleanupRule>, D::Error> {
    let rules: Vec<CleanupRule> = objects(decoder)?;
    let mut ids = BTreeSet::new();
    let mut nodes = 0;
    if !rules
        .iter()
        .all(|rule| ids.insert(&rule.id) && rule.selector.validate(1, &mut nodes))
    {
        return Err(de::Error::custom("invalid cleanup rules"));
    }
    Ok(rules)
}
/// Existing S1 packing/preparation and its exact qualified model contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct S1ChunkStrategy {
    /// Scoped logical metadata.
    #[serde(flatten)]
    pub resource: Resource<ChunkSchema>,
    /// Existing S1 chunker version.
    #[serde(deserialize_with = "bounded_text")]
    #[schemars(length(max = 4096))]
    pub chunker_version: String,
    /// Exact corresponding preparation profile.
    #[serde(deserialize_with = "bounded_text")]
    #[schemars(length(max = 4096))]
    pub preparation_profile: String,
    /// Supported target packing budget.
    pub target_tokens: NonZeroU64,
    /// Maximum complete prepared input, including context and special tokens.
    pub hard_max_tokens: NonZeroU64,
    /// Existing S1 model card pin.
    #[serde(deserialize_with = "object")]
    pub model: Ref,
    /// Exact externally qualified tokenizer evidence.
    #[serde(deserialize_with = "object")]
    pub tokenizer_qualification: Ref,
    /// Checked against the card, never trusted on its own.
    #[serde(deserialize_with = "object")]
    pub model_limits: Limits,
    /// External strategy qualification.
    #[serde(deserialize_with = "object")]
    pub qualification: Ref,
}
/// Text format bounds, without adopting a selector/expression language.
fn bounded_text<'de, D: Deserializer<'de>>(decoder: D) -> Result<String, D::Error> {
    let value = String::deserialize(decoder)?;
    if !shape::valid_text(&value) {
        return Err(de::Error::custom("invalid text"));
    }
    Ok(value)
}
impl S1ChunkStrategy {
    /// Check executable S1 support and independently resolved model evidence.
    /// # Errors
    /// Unsupported budgets/profiles or forged model/tokenizer/limit claims refuse.
    pub fn validate(&self, card: &ModelCard, model: &Ref, tokenizer: &Ref) -> Result<(), Refusal> {
        let profile = ChunkProfile::named(&self.chunker_version).ok_or(Refusal::Unsupported)?;
        if self.preparation_profile != profile.preparation_profile()
            || self.target_tokens.get() != 500
            || self.hard_max_tokens.get() != 700
        {
            return Err(Refusal::Unsupported);
        }
        let identity = card.identity().ok_or(Refusal::Unqualified)?;
        let measured = match &identity.resources.qualified_limits.context_tokens {
            Observation::Measured { value, .. } => *value == self.model_limits.context_tokens,
            Observation::Unavailable { .. } => false,
        };
        if !measured
            || self.hard_max_tokens.get() > u64::from(self.model_limits.context_tokens.get())
            || card.fields().role != Role::Embedder
            || self.model_limits != card.fields().limits
            || self.model != *model
            || self.tokenizer_qualification != *tokenizer
            || card.digest() != &model.digest
            || identity.formats.qualification_digest != tokenizer.digest
        {
            return Err(Refusal::Unqualified);
        }
        Ok(())
    }
}
/// Closed S1 key vocabulary, never executable expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DedupKey {
    /// Original retained bytes.
    OriginalDigest,
    /// Canonical content, separate from occurrences.
    CanonicalDigest,
    /// Complete prepared model input.
    PreparedInputDigest,
    /// Full embedding profile, not just dimensions.
    EmbeddingProfileDigest,
}
/// Executable S1 exact-group and prepared-cache key tuples.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DedupKeys {
    /// Scoped logical metadata.
    #[serde(flatten)]
    pub resource: Resource<DedupSchema>,
    /// Exact tuple, in order.
    #[schemars(length(max = 1000))]
    #[serde(deserialize_with = "keys")]
    pub exact: Vec<DedupKey>,
    /// Prepared cache tuple, in order.
    #[schemars(length(max = 1000))]
    #[serde(deserialize_with = "keys")]
    pub prepared: Vec<DedupKey>,
    /// External qualification pin.
    #[serde(deserialize_with = "object")]
    pub qualification: Ref,
}
impl DedupKeys {
    /// Refuse tuples S1 cannot execute without inventing another dedup engine.
    /// # Errors
    /// Unsupported keys, order, duplicates or lossy combinations refuse.
    pub fn validate(&self) -> Result<(), Refusal> {
        if self.exact != [DedupKey::OriginalDigest, DedupKey::CanonicalDigest]
            || self.prepared
                != [
                    DedupKey::PreparedInputDigest,
                    DedupKey::EmbeddingProfileDigest,
                ]
        {
            return Err(Refusal::Unsupported);
        }
        Ok(())
    }
}

/// Bound key inventories before capability checks.
fn keys<'de, D: Deserializer<'de>>(decoder: D) -> Result<Vec<DedupKey>, D::Error> {
    let values = names(decoder)?;
    if values.len() > 1000 {
        return Err(de::Error::custom("dedup key limit"));
    }
    Ok(values)
}
