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
use std::collections::BTreeMap;

/// Caller-resolved effective values, not a proposal or a stored artifact format.
/// Inputs must come from currently authorized, qualified immutable resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveConfiguration {
    /// Exact immutable baseline identity.
    pub baseline: Ref,
    /// Resolved policy; only source selected-profile leaves may change.
    pub policy: SourcePolicy,
    /// Immutable qualified profile inventory, keyed by definition ID.
    pub profiles: BTreeMap<String, Profile>,
    /// Effective exclusion inventory, keyed by stable decision ID.
    pub decisions: BTreeMap<String, (Ref, Decision)>,
    /// Per-source effective profile, definition and processing values.
    /// The caller resolves profile-default versus collection-selection precedence.
    pub sources: BTreeMap<String, (Ref, ProfileDefinition, Processing)>,
    /// Selected cleanup, chunk and dedup references, respectively.
    /// None leaves profile defaults to the caller's resolver; changes cannot clear a selection.
    pub selected: (Option<Ref>, Option<Ref>, Option<Ref>),
    /// Separately approved cleanup resolutions keyed by reference ID.
    pub approved_cleanup: BTreeMap<String, (Ref, Vec<String>)>,
    /// Separately qualified S1 strategy resolutions keyed by reference ID.
    pub approved_chunks: BTreeMap<String, (Ref, String)>,
    /// Separately approved dedup resolutions keyed by reference ID.
    pub approved_dedup: BTreeMap<String, (Ref, Vec<String>)>,
    /// Qualified S1 hard token maxima, keyed by strategy ID.
    pub qualified_chunk_tokens: BTreeMap<String, u64>,
    /// Separately qualified model maximum, never an adaptive resource budget.
    pub qualified_model_limit: u64,
    /// Exact resolved identities for protected resources outside `SourcePolicy`.
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
            Change::SelectProfile { source_id, profile } => {
                select_profile(&mut expected, candidate, source_id, profile)?;
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
fn selection<'a, T>(
    approved: &'a BTreeMap<String, (Ref, T)>,
    reference: &Ref,
) -> Result<&'a T, WriteError> {
    let (resolved, value) = approved.get(&reference.id).ok_or(WriteError::Held)?;
    if resolved != reference {
        return Err(WriteError::Held);
    }
    Ok(value)
}

/// Caller-resolved precedence may retain a source override or use the selection.
/// It cannot smuggle an unrelated value through a collection-wide edit.
fn set_processing<T: PartialEq + Clone>(
    expected: &mut EffectiveConfiguration,
    candidate: &EffectiveConfiguration,
    selection: &T,
    leaf: fn(&mut Processing) -> &mut T,
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
fn check_chunk(old: &EffectiveConfiguration, chunk: &str) -> Result<(), WriteError> {
    let maximum = old
        .qualified_chunk_tokens
        .get(chunk)
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
    reference: &Ref,
) -> Result<(), WriteError> {
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
            .any(|(_, value)| *value == effective.cleanup);
    let dedup = approved.clone().any(|value| value.dedup == effective.dedup)
        || old
            .approved_dedup
            .values()
            .any(|(_, value)| *value == effective.dedup);
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
