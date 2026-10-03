//! Semantic constraints not expressible by primitive serde shapes.
use super::{
    acquisition::{AcquisitionProfile, ReadyCondition, Transport},
    identity::{FetchIdentity, within},
    resource::Resource,
    schema::SourcePolicy,
    source::{Selector, Source, SyncMode},
};
use crate::{ports::Principal, refusal::Refusal};
use maestro_kernel::scope::Scope;
use maestro_knowledge::collection::Declaration;
use std::collections::BTreeSet;

/// Every resource carries the current collection's exact visibility/scopes.
pub(super) fn resource<S>(
    resource: &Resource<S>,
    collection: &Declaration,
    principal: &Principal<'_>,
) -> Result<(), Refusal> {
    let visibility = serde_json::to_value(collection.visibility).map_err(|_| Refusal::Invalid)?;
    if resource.collection_id != collection.id
        || serde_json::to_value(resource.visibility).map_err(|_| Refusal::Invalid)? != visibility
        || resource.scope_tags.is_empty()
        || principal.id.is_empty()
    {
        return Err(Refusal::Access);
    }
    for tag in &resource.scope_tags {
        let scope: Scope = tag.parse().map_err(|_| Refusal::Invalid)?;
        if !principal.scopes.covers(&scope) {
            return Err(Refusal::Access);
        }
    }
    Ok(())
}

/// Nonempty lists of named records cannot duplicate identities.
pub(super) fn unique<'a>(values: impl Iterator<Item = &'a str>) -> Result<(), Refusal> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(Refusal::Invalid);
        }
    }
    Ok(())
}

/// Validate collection-wide IDs, source counts, budgets and selector bindings.
pub(super) fn policy(policy: &SourcePolicy) -> Result<(), Refusal> {
    if policy.sources.is_empty()
        || policy.sources.len() > 1000
        || policy.acquisition_profiles.is_empty()
        || policy.acquisition_profiles.len() > 1000
        || policy.registries.is_empty()
    {
        return Err(Refusal::Invalid);
    }
    unique(policy.sources.iter().map(|source| source.id.as_str()))?;
    unique(
        policy
            .acquisition_profiles
            .iter()
            .map(|reference| reference.id.as_str()),
    )?;
    unique(
        policy
            .registries
            .iter()
            .map(|reference| reference.id.as_str()),
    )?;
    for source in &policy.sources {
        check_source(source)?;
    }
    Ok(())
}

/// A selector is a nonempty conjunction over one declared source/origin.
pub(crate) fn selector(selector: &Selector, source: &Source) -> Result<(), Refusal> {
    if selector.source_id != source.id
        || selector
            .origin
            .as_ref()
            .is_some_and(|id| !source.origins.iter().any(|origin| &origin.id == id))
    {
        return Err(Refusal::Invalid);
    }
    let lists_empty = selector.object_ids.is_empty()
        && selector.versions.is_empty()
        && selector.channels.is_empty()
        && selector.media_types.is_empty();
    if selector.origin.is_none() && selector.path_prefix.is_none() && lists_empty {
        return Err(Refusal::Invalid);
    }
    if let Some(prefix) = &selector.path_prefix
        && !source
            .origins
            .iter()
            .filter(|origin| selector.origin.as_ref().is_none_or(|id| id == &origin.id))
            .any(|origin| {
                origin
                    .path_prefixes
                    .iter()
                    .any(|allowed| within(prefix, allowed))
            })
    {
        return Err(Refusal::Invalid);
    }
    Ok(())
}

/// Check finite cadence, explicit origin/selection and URL/query semantics.
fn check_source(source: &Source) -> Result<(), Refusal> {
    if source.origins.is_empty()
        || source.seeds.is_empty()
        || source.selectors.is_empty()
        || source.decisions.is_empty()
        || source.selected_profiles.is_empty()
    {
        return Err(Refusal::Invalid);
    }
    unique(source.origins.iter().map(|origin| origin.id.as_str()))?;
    unique(
        source
            .selected_profiles
            .iter()
            .map(|reference| reference.id.as_str()),
    )?;
    for origin in &source.origins {
        if origin.path_prefixes.is_empty() {
            return Err(Refusal::Invalid);
        }
    }
    if source.sync.mode == SyncMode::Watch && source.sync.timer_period_ms.is_none() {
        return Err(Refusal::Invalid);
    }
    if source
        .identity
        .meaningful_queries
        .iter()
        .any(|name| source.identity.ignored_tracking_queries.contains(name))
    {
        return Err(Refusal::Invalid);
    }
    for selector in &source.selectors {
        self::selector(selector, source)?;
    }
    for seed in &source.seeds {
        FetchIdentity::parse(source, seed).map_err(|_| Refusal::Invalid)?;
    }
    Ok(())
}

/// Only rendered profiles may carry bounded declarative readiness.
pub(super) fn profile(profile: &AcquisitionProfile, deadline_ms: u64) -> Result<(), Refusal> {
    match (profile.transport, &profile.readiness) {
        (Transport::Http | Transport::BrowserRequest, None) => Ok(()),
        (Transport::BrowserRender, Some(ready)) => {
            if ready.all_of.is_empty()
                || ready.all_of.len() > 32
                || ready.timeout_ms.get() > deadline_ms
                || ready.poll_interval_ms > ready.timeout_ms
                || ready.stable_for_ms > ready.timeout_ms
            {
                return Err(Refusal::Invalid);
            }
            for condition in &ready.all_of {
                ready_condition(condition)?;
            }
            Ok(())
        }
        _ => Err(Refusal::Invalid),
    }
}

/// Bounded exact tag/attribute paths, never executable selector languages.
fn ready_condition(condition: &ReadyCondition) -> Result<(), Refusal> {
    let path = match condition {
        ReadyCondition::ElementPresent { selector }
        | ReadyCondition::ElementText { selector, .. }
        | ReadyCondition::ElementAbsent { selector } => selector,
    };
    if path.is_empty() || path.len() > 32 {
        return Err(Refusal::Invalid);
    }
    for step in path {
        if step.attributes.len() > 8 {
            return Err(Refusal::Invalid);
        }
        unique(
            step.attributes
                .iter()
                .map(|attribute| attribute.name.as_str()),
        )?;
    }
    Ok(())
}

/// Two conjunctive selectors intersect only when every present dimension can.
pub(super) fn overlap(left: &Selector, right: &Selector) -> bool {
    left.source_id == right.source_id
        && (left.origin.is_none() || right.origin.is_none() || left.origin == right.origin)
        && match (&left.path_prefix, &right.path_prefix) {
            (Some(left), Some(right)) => within(left, right) || within(right, left),
            _ => true,
        }
        && list_overlap(&left.object_ids, &right.object_ids)
        && list_overlap(&left.versions, &right.versions)
        && list_overlap(&left.channels, &right.channels)
        && list_overlap(&left.media_types, &right.media_types)
}

/// Empty lists are absent dimensions; nonempty values are disjunctions.
fn list_overlap(left: &[String], right: &[String]) -> bool {
    left.is_empty() || right.is_empty() || left.iter().any(|value| right.contains(value))
}
