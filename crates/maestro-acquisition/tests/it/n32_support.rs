//! Pure synthetic N32 controls; runtime artifact contracts belong to N34.
#![expect(clippy::indexing_slicing, reason = "authored synthetic keys")]
use maestro_acquisition::adaptation::change::selection_key;
use maestro_acquisition::{
    Ref,
    adaptation::{
        Change, WriteError,
        change::{EffectiveConfiguration, admit, apply},
    },
    extraction::{
        model::{Profile, ProfileDefinition},
        registry::definition_bytes,
    },
    policy::{
        decisions::{Action, Decisions},
        manifest::AutomaticClass,
        schema::SourcePolicy,
    },
};
use maestro_kernel::artifact::Digest;
use std::collections::BTreeMap;

pub(super) const NOW: &str = "2026-10-01T00:00:00Z";

pub(super) fn reference(id: &str) -> Ref {
    Ref {
        id: id.into(),
        digest: Digest::of(id.as_bytes()),
    }
}

pub(super) fn fixture() -> EffectiveConfiguration {
    let mut policy: SourcePolicy =
        serde_json::from_str(include_str!("../fixtures/policy.json")).unwrap();
    policy.adaptation.automatic_classes = vec![
        AutomaticClass::SelectedProfiles,
        AutomaticClass::Cleanup,
        AutomaticClass::S1ChunkStrategy,
        AutomaticClass::DedupKeys,
        AutomaticClass::NewKnowledgeExclusions,
    ];
    let text = include_str!("../fixtures/profile-definition-v2.txt")
        .split_once('\n')
        .unwrap()
        .1;
    let mut definition: ProfileDefinition = serde_json::from_str(text).unwrap();
    definition.id = "old".into();
    let old = Profile {
        definition_digest: Digest::of(&definition_bytes(&definition).unwrap()),
        definition: definition.clone(),
    };
    definition.id = "new".into();
    let new = Profile {
        definition_digest: Digest::of(&definition_bytes(&definition).unwrap()),
        definition,
    };
    policy.sources[0].selected_profiles = vec![old.reference()];
    let processing = old.definition.processing.clone();
    let selected = (
        Some(reference("cleanup-old")),
        Some(reference("chunk-old")),
        Some(reference("dedup-old")),
    );
    let entries: Decisions =
        serde_json::from_str(include_str!("../fixtures/decisions.json")).unwrap();
    EffectiveConfiguration {
        baseline: reference("baseline"),
        policy,
        profiles: BTreeMap::from([("old".into(), old.clone()), ("new".into(), new)]),
        decisions: entries
            .entries
            .into_iter()
            .map(|entry| (entry.id.clone(), (reference(&entry.id), entry)))
            .collect(),
        sources: BTreeMap::from([(
            "notes".into(),
            (old.reference(), old.definition.clone(), processing),
        )]),
        selected,
        approved_cleanup: BTreeMap::from([("cleanup-new".into(), reference("cleanup-new"))]),
        approved_chunks: BTreeMap::from([("chunk-new".into(), reference("chunk-new"))]),
        approved_dedup: BTreeMap::from([("dedup-new".into(), reference("dedup-new"))]),
        qualified_chunk_tokens: BTreeMap::from([
            ("chunk-new".into(), 700),
            ("synthetic-chunk".into(), 700),
        ]),
        qualified_model_limit: 700,
        protected_resources: BTreeMap::new(),
    }
}

pub(super) fn disjoint_fixture() -> EffectiveConfiguration {
    let mut old = fixture();
    old.decisions
        .get_mut("no-assets")
        .unwrap()
        .1
        .selector
        .path_prefix = Some("/docs/assets".into());
    old
}

pub(super) fn disjoint_exclusion(
    old: &EffectiveConfiguration,
    action: Action,
) -> (EffectiveConfiguration, Change) {
    let (mut candidate, change) = exclusion(old, action);
    candidate
        .decisions
        .get_mut("new-exclusion")
        .unwrap()
        .1
        .selector
        .path_prefix = Some("/docs/text".into());
    (candidate, change)
}

pub(super) fn cleanup() -> Change {
    Change::SetCleanup {
        rules: reference("cleanup-new"),
    }
}

pub(super) fn resolved(old: &EffectiveConfiguration, change: &Change) -> EffectiveConfiguration {
    let mut candidate = old.clone();
    match change {
        Change::SelectProfile {
            source_id,
            profile,
            selection,
        } => {
            let definition = &old.profiles[&profile.id].definition;
            candidate
                .protected_resources
                .insert(selection_key(source_id), selection.clone());
            candidate.sources.insert(
                source_id.clone(),
                (
                    profile.clone(),
                    definition.clone(),
                    definition.processing.clone(),
                ),
            );
        }
        Change::SetCleanup { rules } => {
            for (_, _, processing) in candidate.sources.values_mut() {
                processing
                    .cleanup
                    .clone_from(&old.approved_cleanup[&rules.id]);
            }
        }
        Change::SetS1ChunkStrategy { strategy } => {
            for (_, _, processing) in candidate.sources.values_mut() {
                processing
                    .chunk
                    .clone_from(&old.approved_chunks[&strategy.id]);
            }
        }
        Change::SetDedupKeys { keys } => {
            for (_, _, processing) in candidate.sources.values_mut() {
                processing.dedup.clone_from(&old.approved_dedup[&keys.id]);
            }
        }
        _ => {}
    }
    candidate
}

pub(super) fn held(
    old: &EffectiveConfiguration,
    candidate: &EffectiveConfiguration,
    changes: &[Change],
) {
    let mut candidate = candidate.clone();
    // Preserve the forbidden diff while making its accompanying allowed leaves exact.
    if let Ok(expected) = apply(old, &candidate, changes, NOW) {
        for (id, (_, _, processing)) in &mut candidate.sources {
            processing.clone_from(&expected.sources[id].2);
        }
        candidate.selected = expected.selected;
    }
    assert_eq!(
        admit(old, &candidate, changes, NOW, false),
        Err(WriteError::Held)
    );
}

pub(super) fn exclusion(
    old: &EffectiveConfiguration,
    action: Action,
) -> (EffectiveConfiguration, Change) {
    let mut candidate = old.clone();
    let mut entry = old.decisions["no-assets"].1.clone();
    entry.id = "new-exclusion".into();
    entry.action = action;
    candidate
        .decisions
        .insert(entry.id.clone(), (reference(&entry.id), entry));
    let change = if action == Action::AssetOnly {
        Change::AddAssetOnly {
            entry: reference("new-exclusion"),
        }
    } else {
        Change::AddKnowledgeExclusion {
            entry: reference("new-exclusion"),
        }
    };
    (candidate, change)
}
