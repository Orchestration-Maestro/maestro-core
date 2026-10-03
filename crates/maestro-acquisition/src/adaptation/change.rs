//! Pure closed automatic-change control over freshly resolved, typed values.
use super::manifest::{Change, WriteError};
use crate::{
    Ref,
    extraction::model::{Processing, Profile, ProfileDefinition, QualificationState},
    policy::{
        checks,
        decision::time_key,
        decisions::{Action, Decision},
        manifest::AutomaticClass,
        resolve::check_conflicts,
        schema::SourcePolicy,
        shape,
    },
};
use maestro_kernel::artifact::Digest;
use maestro_knowledge::strict_json::{nullable_object, object};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::collections::BTreeMap;

/// Cleanup/chunk/dedup collection selections, with explicit null defaults.
pub type ProcessingSelections = (Option<Ref>, Option<Ref>, Option<Ref>);

/// Existing per-source effective triples, serialized as arrays.
pub type SourceConfigurations = BTreeMap<String, (Ref, ProfileDefinition, Processing)>;
/// Existing decision identities and definitions, serialized as arrays.
pub type DecisionInventory = BTreeMap<String, (Ref, Decision)>;

/// Deterministic bounded logical key for a source's retained selection receipt.
/// All IDs use their digest so the whole key obeys N03's bound without namespaces.
#[must_use]
pub fn selection_key(source_id: &str) -> String {
    format!(
        "profile_selection.{}",
        Digest::of(source_id.as_bytes()).as_str()
    )
}

/// Caller-resolved effective values, not a proposal or a stored artifact format.
/// Inputs must come from currently authorized, qualified immutable resources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectiveConfiguration {
    /// Exact immutable baseline identity.
    #[serde(deserialize_with = "object")]
    pub baseline: Ref,
    /// Resolved policy; only source selected-profile leaves may change.
    #[serde(deserialize_with = "object")]
    pub policy: SourcePolicy,
    /// Per-source effective profile, definition and processing values.
    #[serde(deserialize_with = "source_map")]
    #[schemars(length(max = 1000))]
    pub sources: SourceConfigurations,
    /// Immutable qualified profile inventory, keyed by definition ID.
    #[serde(deserialize_with = "object_map")]
    #[schemars(length(max = 1000))]
    pub profiles: BTreeMap<String, Profile>,
    /// Effective exclusion inventory, keyed by stable decision ID.
    #[serde(deserialize_with = "decision_map")]
    #[schemars(length(max = 1000))]
    pub decisions: DecisionInventory,
    /// Selected cleanup, chunk and dedup references, respectively.
    /// None leaves profile defaults to the caller's resolver; changes cannot clear a selection.
    #[serde(deserialize_with = "selected_pins")]
    pub selected: ProcessingSelections,
    /// Separately approved cleanup resolutions keyed by reference ID.
    #[serde(deserialize_with = "object_map")]
    #[schemars(length(max = 1000))]
    pub approved_cleanup: BTreeMap<String, Ref>,
    /// Separately qualified S1 strategy resolutions keyed by reference ID.
    #[serde(deserialize_with = "object_map")]
    #[schemars(length(max = 1000))]
    pub approved_chunks: BTreeMap<String, Ref>,
    /// Separately approved dedup resolutions keyed by reference ID.
    #[serde(deserialize_with = "object_map")]
    #[schemars(length(max = 1000))]
    pub approved_dedup: BTreeMap<String, Ref>,
    /// Qualified S1 hard token maxima, keyed by strategy ID.
    #[serde(deserialize_with = "bounded_map")]
    #[schemars(length(max = 1000))]
    pub qualified_chunk_tokens: BTreeMap<String, u64>,
    /// Separately qualified model maximum, never an adaptive resource budget.
    pub qualified_model_limit: u64,
    /// Exact resolved identities for protected resources outside `SourcePolicy`.
    #[serde(deserialize_with = "object_map")]
    #[schemars(length(max = 1000))]
    pub protected_resources: BTreeMap<String, Ref>,
}

/// Build an expected snapshot without mutating either input.
/// # Errors
/// Unresolved, unqualified, disabled or non-narrowing selections hold.
pub fn apply(
    old: &EffectiveConfiguration,
    candidate: &EffectiveConfiguration,
    changes: &[Change],
    now: &str,
) -> Result<EffectiveConfiguration, WriteError> {
    if changes.is_empty() || !shape::valid_time(now) {
        return Err(WriteError::Held);
    }
    let mut expected = old.clone();
    for change in changes {
        let class = class(change);
        if !old.policy.adaptation.automatic_classes.contains(&class) {
            return Err(WriteError::Held);
        }
        match change {
            Change::SelectProfile {
                source_id,
                profile,
                selection,
            } => {
                select_profile(&mut expected, candidate, source_id, (profile, selection))?;
            }
            Change::SetCleanup { rules } => {
                set_processing(
                    &mut expected,
                    candidate,
                    selection(&old.approved_cleanup, rules)?,
                    |value| &mut value.cleanup,
                )?;
                expected.selected.0 = Some(rules.clone());
            }
            Change::SetS1ChunkStrategy { strategy } => {
                check_chunk(old, selection(&old.approved_chunks, strategy)?)?;
                set_processing(
                    &mut expected,
                    candidate,
                    selection(&old.approved_chunks, strategy)?,
                    |value| &mut value.chunk,
                )?;
                expected.selected.1 = Some(strategy.clone());
            }
            Change::SetDedupKeys { keys } => {
                set_processing(
                    &mut expected,
                    candidate,
                    selection(&old.approved_dedup, keys)?,
                    |value| &mut value.dedup,
                )?;
                expected.selected.2 = Some(keys.clone());
            }
            Change::AddKnowledgeExclusion { entry } => {
                add_exclusion(
                    &mut expected,
                    candidate,
                    entry,
                    Action::ExcludeFromKnowledge,
                    now,
                )?;
            }
            Change::AddAssetOnly { entry } => {
                add_exclusion(&mut expected, candidate, entry, Action::AssetOnly, now)?;
            }
        }
    }
    Ok(expected)
}

/// Typed closed mapping; unknown wire variants never decode as Change.
fn class(change: &Change) -> AutomaticClass {
    match change {
        Change::SelectProfile { .. } => AutomaticClass::SelectedProfiles,
        Change::SetCleanup { .. } => AutomaticClass::Cleanup,
        Change::SetS1ChunkStrategy { .. } => AutomaticClass::S1ChunkStrategy,
        Change::SetDedupKeys { .. } => AutomaticClass::DedupKeys,
        Change::AddKnowledgeExclusion { .. } | Change::AddAssetOnly { .. } => {
            AutomaticClass::NewKnowledgeExclusions
        }
    }
}

/// A resolved name cannot silently substitute another immutable reference.
fn selection<'a>(
    approved: &'a BTreeMap<String, Ref>,
    reference: &Ref,
) -> Result<&'a Ref, WriteError> {
    let resolved = approved.get(&reference.id).ok_or(WriteError::Held)?;
    if resolved != reference {
        return Err(WriteError::Held);
    }
    Ok(resolved)
}

/// Caller-resolved precedence may retain a source override or use the selection.
/// It cannot smuggle an unrelated value through a collection-wide edit.
fn set_processing(
    expected: &mut EffectiveConfiguration,
    candidate: &EffectiveConfiguration,
    selection: &Ref,
    leaf: fn(&mut Processing) -> &mut Ref,
) -> Result<(), WriteError> {
    for (source_id, (_, _, effective)) in &mut expected.sources {
        let mut resolved = candidate
            .sources
            .get(source_id)
            .ok_or(WriteError::Held)?
            .2
            .clone();
        let current = leaf(effective);
        let proposed = leaf(&mut resolved);
        if proposed != current && proposed != selection {
            return Err(WriteError::Held);
        }
        current.clone_from(proposed);
    }
    Ok(())
}

/// S1 qualifications are explicit inputs, never inferred from strategy names.
fn check_chunk(old: &EffectiveConfiguration, chunk: &Ref) -> Result<(), WriteError> {
    let maximum = old
        .qualified_chunk_tokens
        .get(&chunk.id)
        .ok_or(WriteError::Held)?;
    if *maximum == 0 || *maximum > old.qualified_model_limit {
        return Err(WriteError::Held);
    }
    Ok(())
}

/// Compare resolved protected values across every prior selected profile.
fn select_profile(
    expected: &mut EffectiveConfiguration,
    candidate: &EffectiveConfiguration,
    source_id: &str,
    pins: (&Ref, &Ref),
) -> Result<(), WriteError> {
    let (reference, selection) = pins;
    let profile = expected
        .profiles
        .get(&reference.id)
        .ok_or(WriteError::Held)?;
    if profile.reference() != *reference
        || profile.definition.qualification_state != QualificationState::Qualified
    {
        return Err(WriteError::Held);
    }
    let resolved = candidate.sources.get(source_id).ok_or(WriteError::Held)?;
    let previous = expected.sources.get(source_id).ok_or(WriteError::Held)?;
    if resolved.0 != *reference || resolved.1 != profile.definition {
        return Err(WriteError::Held);
    }
    if !previous.1.same_protected_fields(&resolved.1) {
        return Err(WriteError::Held);
    }
    check_processing(expected, &resolved.2)?;
    let source = expected
        .policy
        .sources
        .iter_mut()
        .find(|source| source.id == source_id)
        .ok_or(WriteError::Held)?;
    source.selected_profiles = vec![reference.clone()];
    expected.sources.insert(source_id.into(), resolved.clone());
    expected
        .protected_resources
        .insert(selection_key(source_id), selection.clone());
    Ok(())
}

/// Effective processing must stay within the separately approved ID inventory.
fn check_processing(
    old: &EffectiveConfiguration,
    effective: &Processing,
) -> Result<(), WriteError> {
    check_chunk(old, &effective.chunk)?;
    let approved = old
        .profiles
        .values()
        .map(|profile| &profile.definition.processing);
    let cleanup = approved
        .clone()
        .any(|value| value.cleanup == effective.cleanup)
        || old
            .approved_cleanup
            .values()
            .any(|value| *value == effective.cleanup);
    let dedup = approved.clone().any(|value| value.dedup == effective.dedup)
        || old
            .approved_dedup
            .values()
            .any(|value| *value == effective.dedup);
    if !cleanup || !dedup {
        return Err(WriteError::Held);
    }
    Ok(())
}

/// Additive restrictive decisions cannot replace an earlier stable ID or action.
fn add_exclusion(
    expected: &mut EffectiveConfiguration,
    candidate: &EffectiveConfiguration,
    reference: &Ref,
    action: Action,
    now: &str,
) -> Result<(), WriteError> {
    let (id, (resolved, entry)) = candidate
        .decisions
        .iter()
        .find(|(_, (resolved, _))| resolved == reference)
        .ok_or(WriteError::Held)?;
    if entry.id != *id || entry.action != action {
        return Err(WriteError::Held);
    }
    if expected.decisions.contains_key(&entry.id) {
        return Err(WriteError::Held);
    }
    let source = expected
        .policy
        .sources
        .iter()
        .find(|source| source.id == entry.selector.source_id)
        .ok_or(WriteError::Held)?;
    checks::selector(&entry.selector, source).map_err(|_| WriteError::Held)?;
    if !shape::valid_time(&entry.effective_at)
        || time_key(&entry.effective_at) > time_key(now)
        || entry
            .expires_at
            .as_deref()
            .is_some_and(|expiry| !shape::valid_time(expiry) || time_key(expiry) <= time_key(now))
    {
        return Err(WriteError::Held);
    }
    let previous = expected
        .decisions
        .values()
        .map(|(_, entry)| entry)
        .collect::<Vec<_>>();
    check_conflicts(&previous, entry).map_err(|_| WriteError::Held)?;
    expected
        .decisions
        .insert(entry.id.clone(), (resolved.clone(), entry.clone()));
    Ok(())
}

/// Admit the entire diff, not just its named changes, before quality gates.
/// OA1 contains no N/A cells; claimed N/A cannot grant automatic permission.
/// # Errors
/// Any protected difference or unauthorized N/A holds the whole candidate.
pub fn admit(
    old: &EffectiveConfiguration,
    candidate: &EffectiveConfiguration,
    changes: &[Change],
    now: &str,
    not_applicable: bool,
) -> Result<(), WriteError> {
    if not_applicable {
        return Err(WriteError::Held);
    }
    let expected = apply(old, candidate, changes, now)?;
    // Full typed equality closes the complement, including new snapshot fields.
    if expected != *candidate {
        return Err(WriteError::Held);
    }
    Ok(())
}

/// Bound logical inventory keys independently of the enclosing JSON limit.
fn bounded_map<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    decoder: D,
) -> Result<BTreeMap<String, T>, D::Error> {
    let map = BTreeMap::<String, T>::deserialize(decoder)?;
    if map.len() > 1000 || !map.keys().all(|id| shape::valid_id(id)) {
        return Err(de::Error::custom("invalid effective inventory"));
    }
    Ok(map)
}
/// One nullable object pin; tuple slots stay explicit nulls on the wire.
#[derive(Deserialize)]
struct NullablePin(#[serde(deserialize_with = "nullable_object")] Option<Ref>);
/// Prevent positional Ref arrays inside the three selection slots.
fn selected_pins<'de, D: Deserializer<'de>>(decoder: D) -> Result<ProcessingSelections, D::Error> {
    let (cleanup, chunk, dedup) = <(NullablePin, NullablePin, NullablePin)>::deserialize(decoder)?;
    Ok((cleanup.0, chunk.0, dedup.0))
}

/// Object shape only, without introducing another effective/wire DTO.
#[derive(Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct ObjectValue<T>(#[serde(deserialize_with = "object")] T);
/// Bounded maps whose values are strict objects.
fn object_map<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    decoder: D,
) -> Result<BTreeMap<String, T>, D::Error> {
    let values: BTreeMap<String, ObjectValue<T>> = bounded_map(decoder)?;
    Ok(values
        .into_iter()
        .map(|(key, value)| (key, value.0))
        .collect())
}
/// Object-only decoder components for the existing triple.
type SourceObjects = (
    ObjectValue<Ref>,
    ObjectValue<ProfileDefinition>,
    ObjectValue<Processing>,
);
/// Strict object components inside the existing array triples.
fn source_map<'de, D: Deserializer<'de>>(decoder: D) -> Result<SourceConfigurations, D::Error> {
    let values: BTreeMap<String, SourceObjects> = bounded_map(decoder)?;
    Ok(values
        .into_iter()
        .map(|(key, (pin, definition, processing))| (key, (pin.0, definition.0, processing.0)))
        .collect())
}
/// Strict object components inside the existing decision pairs.
fn decision_map<'de, D: Deserializer<'de>>(decoder: D) -> Result<DecisionInventory, D::Error> {
    let values: BTreeMap<String, (ObjectValue<Ref>, ObjectValue<Decision>)> = bounded_map(decoder)?;
    Ok(values
        .into_iter()
        .map(|(key, (pin, decision))| (key, (pin.0, decision.0)))
        .collect())
}
