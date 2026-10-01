//! URL identity is explicit, versioned and separate from protected references.
use super::{
    resource::Resource,
    shape,
    source::{IdentityRule, QueryOrder, RepeatedQueries, Source},
};
use crate::refusal::Refusal;
use reqwest::Url;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt, num::NonZeroU64};

/// A policy-bound stable fetch key, never a display or transient transfer URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchIdentity {
    /// Exact declared source namespace.
    source_id: String,
    /// Exact declared origin, including its port.
    origin_id: String,
    /// Rule version participates in key equality.
    version: NonZeroU64,
    /// Fragment-free URL containing only meaningful raw query pairs.
    url: Url,
}
impl FetchIdentity {
    /// Parse once under explicit HTTPS origin/path and query semantics.
    ///
    /// # Errors
    /// Ambiguous URLs, undeclared origins/paths or query semantics refuse.
    pub fn parse(source: &Source, text: &str) -> Result<Self, Refusal> {
        let mut url = shape::checked_url(text).ok_or(Refusal::Invalid)?;
        let mut origins = source.origins.iter().filter(|origin| {
            url.host_str() == Some(origin.host.as_str())
                && url.port_or_known_default() == Some(origin.port.get())
                && origin
                    .path_prefixes
                    .iter()
                    .any(|prefix| within(url.path(), prefix))
        });
        let origin = origins.next().ok_or(Refusal::Access)?;
        if origins.next().is_some() {
            return Err(Refusal::Invalid);
        }
        let query = meaningful_query(&url, &source.identity)?;
        url.set_query(query.as_deref());
        url.set_fragment(None);
        Ok(Self {
            source_id: source.id.clone(),
            origin_id: origin.id.clone(),
            version: source.identity.version,
            url,
        })
    }
    /// Stable URL spelling; source and rule version remain separate key fields.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }
    /// Declared source namespace.
    #[must_use]
    pub fn source_id(&self) -> &str {
        &self.source_id
    }
    /// Matched declared origin ID.
    #[must_use]
    pub fn origin_id(&self) -> &str {
        &self.origin_id
    }
    /// Explicit rule version; a changed version never silently aliases keys.
    #[must_use]
    pub fn version(&self) -> NonZeroU64 {
        self.version
    }
    /// Parsed URL for checked-address and robots adapters; immutable to callers.
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }
}

/// Segment boundaries, not string-prefix authority.
pub(super) fn within(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| prefix.ends_with('/') || rest.starts_with('/'))
}

/// Decode names for classification only; preserve raw value, escaping and `=`.
fn meaningful_query(url: &Url, rule: &IdentityRule) -> Result<Option<String>, Refusal> {
    let meaningful: BTreeSet<_> = rule.meaningful_queries.iter().collect();
    let ignored: BTreeSet<_> = rule.ignored_tracking_queries.iter().collect();
    if !meaningful.is_disjoint(&ignored)
        || meaningful.len() != rule.meaningful_queries.len()
        || ignored.len() != rule.ignored_tracking_queries.len()
    {
        return Err(Refusal::Invalid);
    }
    let Some(query) = url.query() else {
        return Ok(None);
    };
    let mut seen = BTreeSet::new();
    let mut retained = Vec::new();
    for (raw, (name, _)) in query.split('&').zip(url.query_pairs()) {
        if name.is_empty() {
            return Err(Refusal::Invalid);
        }
        if rule.repeated_queries == RepeatedQueries::Reject && !seen.insert(name.to_string()) {
            return Err(Refusal::Invalid);
        }
        if meaningful.contains(&name.to_string()) {
            retained.push((name.to_string(), raw));
        } else if !ignored.contains(&name.to_string()) {
            return Err(Refusal::Invalid);
        }
    }
    // query_pairs skips empty pairs, so check them independently before zipping.
    if query.split('&').any(str::is_empty) {
        return Err(Refusal::Invalid);
    }
    if rule.query_order == QueryOrder::Sort {
        retained.sort_by(|left, right| left.0.cmp(&right.0));
    }
    if retained.is_empty() {
        Ok(None)
    } else {
        Ok(Some(
            retained
                .iter()
                .map(|(_, raw)| *raw)
                .collect::<Vec<_>>()
                .join("&"),
        ))
    }
}

/// A display link retains its fragment and original spelling, not fetch authority.
#[derive(Clone, PartialEq, Eq)]
pub struct DisplayLink(String);
impl DisplayLink {
    /// Validate syntax without converting this reference into a fetch identity.
    ///
    /// # Errors
    /// Unsafe or ambiguous HTTPS references refuse.
    pub fn parse(text: &str) -> Result<Self, Refusal> {
        shape::checked_url(text).ok_or(Refusal::Invalid)?;
        Ok(Self(text.to_owned()))
    }
    /// Protected original display reference, for an authorized local view only.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for DisplayLink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DisplayLink([protected])")
    }
}

/// A protected transient transfer URL; never a stable fetch or asset identity.
#[derive(Clone, PartialEq, Eq)]
pub struct SignedTransferUrl(String);
impl SignedTransferUrl {
    /// Validate syntax only; each transfer still needs fresh destination admission.
    ///
    /// # Errors
    /// Unsafe or ambiguous HTTPS references refuse.
    pub fn parse(text: &str) -> Result<Self, Refusal> {
        shape::checked_url(text).ok_or(Refusal::Invalid)?;
        Ok(Self(text.to_owned()))
    }
    /// Protected transient URL, supplied only to an admitted transfer adapter.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for SignedTransferUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SignedTransferUrl([protected])")
    }
}

/// The only supported migration resource version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum MigrationSchema {
    /// Version one retains explicit old/new mappings without applying aliases.
    #[serde(rename = "maestro-url-identity-migration/1")]
    V1,
}
/// One reviewed immutable mapping; no guessed normalization or permission union.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdentityMapping {
    /// Exact historical stable URL spelling.
    #[serde(deserialize_with = "shape::url")]
    pub old: String,
    /// Exact replacement stable URL spelling, re-admitted under current policy.
    #[serde(deserialize_with = "shape::url")]
    pub new: String,
}
/// Persisted through the same scoped, reviewed immutable-resource adapter as policy.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdentityMigration {
    /// Common owner, version, collection and scopes.
    #[serde(flatten)]
    pub resource: Resource<MigrationSchema>,
    /// Source namespace; mappings never join sources or permissions.
    #[serde(deserialize_with = "shape::id")]
    pub source_id: String,
    /// Historical identity-rule version.
    pub old_version: NonZeroU64,
    /// Reviewed replacement identity-rule version.
    pub new_version: NonZeroU64,
    /// Bounded explicit old-to-new entries, retained even after future migrations.
    #[serde(deserialize_with = "shape::objects")]
    #[schemars(length(min = 1, max = 10000))]
    pub entries: Vec<IdentityMapping>,
}
impl IdentityMigration {
    /// Check source/version, exact old keys and replacement URL identity.
    pub(super) fn validate(&self, source: &Source) -> Result<(), Refusal> {
        if self.source_id != source.id
            || self.old_version >= self.new_version
            || self.new_version != source.identity.version
            || self.entries.is_empty()
        {
            return Err(Refusal::Invalid);
        }
        let mut old_keys = BTreeSet::new();
        for entry in &self.entries {
            let new = FetchIdentity::parse(source, &entry.new)?;
            if entry.old.contains('#') || new.as_str() != entry.new || !old_keys.insert(&entry.old)
            {
                return Err(Refusal::Invalid);
            }
        }
        if self
            .entries
            .iter()
            .any(|entry| old_keys.contains(&entry.new))
        {
            return Err(Refusal::Invalid);
        }
        Ok(())
    }
}
