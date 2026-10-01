//! Isolated boundary probes for N32's pure controls.
use super::n32_support::{
    NOW, cleanup, disjoint_exclusion, disjoint_fixture, exclusion, fixture, reference, resolved,
};
use maestro_acquisition::{
    adaptation::{
        Change, WriteError,
        change::{admit, apply},
    },
    extraction::model::QualificationState,
    policy::{decisions::Action, manifest::AutomaticClass},
};
use std::slice::from_ref;

#[test]
fn n32_exclusion_artifact_ref_differs_from_stable_id() {
    let old = disjoint_fixture();
    for action in [Action::AssetOnly, Action::ExcludeFromKnowledge] {
        let (mut candidate, _) = disjoint_exclusion(&old, action);
        let (artifact, _) = candidate.decisions.get_mut("new-exclusion").unwrap();
        *artifact = reference("artifact-handle");
        let change = if action == Action::AssetOnly {
            Change::AddAssetOnly {
                entry: artifact.clone(),
            }
        } else {
            Change::AddKnowledgeExclusion {
                entry: artifact.clone(),
            }
        };
        assert_eq!(
            apply(&old, &candidate, from_ref(&change), NOW),
            Ok(candidate.clone())
        );
        assert_eq!(
            admit(&old, &candidate, from_ref(&change), NOW, false),
            Ok(())
        );
    }
}

#[test]
fn n32_stable_decision_id_cannot_be_reused() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate.decisions.get_mut("no-assets").unwrap().0 = reference("another-artifact");
    let change = Change::AddAssetOnly {
        entry: reference("another-artifact"),
    };
    assert_eq!(
        apply(&old, &candidate, from_ref(&change), NOW).map(|_| ()),
        Err(WriteError::Held)
    );
    assert_eq!(
        admit(&old, &candidate, from_ref(&change), NOW, false),
        Err(WriteError::Held)
    );
}

#[test]
fn n32_new_exclusion_cannot_conflict_with_existing_disposition() {
    let old = fixture();
    let (candidate, change) = exclusion(&old, Action::ExcludeFromKnowledge);
    assert_eq!(
        apply(&old, &candidate, from_ref(&change), NOW).map(|_| ()),
        Err(WriteError::Held)
    );
    assert_eq!(
        admit(&old, &candidate, from_ref(&change), NOW, false),
        Err(WriteError::Held)
    );

    let (candidate, change) = exclusion(&old, Action::AssetOnly);
    assert_eq!(
        apply(&old, &candidate, from_ref(&change), NOW),
        Ok(candidate.clone())
    );
    assert_eq!(
        admit(&old, &candidate, from_ref(&change), NOW, false),
        Ok(())
    );
    let old = disjoint_fixture();
    let (candidate, change) = disjoint_exclusion(&old, Action::ExcludeFromKnowledge);
    assert_eq!(
        apply(&old, &candidate, from_ref(&change), NOW),
        Ok(candidate.clone())
    );
    assert_eq!(
        admit(&old, &candidate, from_ref(&change), NOW, false),
        Ok(())
    );
}

#[test]
fn n32_exclusions_cannot_conflict_with_earlier_proposal_entries() {
    let old = disjoint_fixture();
    for action in [Action::AssetOnly, Action::ExcludeFromKnowledge] {
        let (mut candidate, first) = disjoint_exclusion(&old, action);
        let mut second = candidate.decisions["new-exclusion"].1.clone();
        second.id = "second-exclusion".into();
        second.action = if action == Action::AssetOnly {
            Action::ExcludeFromKnowledge
        } else {
            Action::AssetOnly
        };
        let artifact = reference("second-artifact");
        let change = if second.action == Action::AssetOnly {
            Change::AddAssetOnly {
                entry: artifact.clone(),
            }
        } else {
            Change::AddKnowledgeExclusion {
                entry: artifact.clone(),
            }
        };
        candidate
            .decisions
            .insert(second.id.clone(), (artifact, second));
        let changes = [first, change];
        assert_eq!(
            apply(&old, &candidate, &changes, NOW).map(|_| ()),
            Err(WriteError::Held)
        );
        assert_eq!(
            admit(&old, &candidate, &changes, NOW, false),
            Err(WriteError::Held)
        );
    }
}

#[test]
fn n32_exact_selection_targets() {
    for absent in [false, true] {
        let mut old = fixture();
        if absent {
            old.selected = (None, None, None);
        }
        for change in [
            Change::SelectProfile {
                source_id: "notes".into(),
                profile: old.profiles["new"].reference(),
                selection: reference("selection-new"),
            },
            cleanup(),
            Change::SetS1ChunkStrategy {
                strategy: reference("chunk-new"),
            },
            Change::SetDedupKeys {
                keys: reference("dedup-new"),
            },
        ] {
            let mut candidate = resolved(&old, &change);
            match &change {
                Change::SelectProfile { profile, .. } => {
                    candidate.policy.sources[0].selected_profiles = vec![profile.clone()];
                }
                Change::SetCleanup { rules } => candidate.selected.0 = Some(rules.clone()),
                Change::SetS1ChunkStrategy { strategy } => {
                    candidate.selected.1 = Some(strategy.clone());
                }
                Change::SetDedupKeys { keys } => candidate.selected.2 = Some(keys.clone()),
                Change::AddKnowledgeExclusion { .. } | Change::AddAssetOnly { .. } => {}
            }
            assert_eq!(
                apply(&old, &candidate, from_ref(&change), NOW),
                Ok(candidate.clone())
            );
            assert_eq!(
                admit(&old, &candidate, from_ref(&change), NOW, false),
                Ok(())
            );
        }
    }
}

#[test]
fn n32_exclusions_require_enabled_class() {
    let mut old = disjoint_fixture();
    old.policy.adaptation.automatic_classes.clear();
    for action in [Action::AssetOnly, Action::ExcludeFromKnowledge] {
        let (mut candidate, change) = disjoint_exclusion(&old, action);
        assert_eq!(
            apply(&old, &candidate, from_ref(&change), NOW).map(|_| ()),
            Err(WriteError::Held)
        );
        let mut enabled = old.clone();
        enabled.policy.adaptation.automatic_classes = vec![AutomaticClass::NewKnowledgeExclusions];
        candidate
            .policy
            .adaptation
            .automatic_classes
            .clone_from(&enabled.policy.adaptation.automatic_classes);
        assert_eq!(
            apply(&enabled, &candidate, from_ref(&change), NOW),
            Ok(candidate.clone())
        );
        assert_eq!(
            admit(&enabled, &candidate, from_ref(&change), NOW, false),
            Ok(())
        );
    }
}

#[test]
fn n32_some_selection_cannot_be_cleared() {
    let old = fixture();
    for group in 0..3 {
        let changes = [Change::AddAssetOnly {
            entry: reference("new-exclusion"),
        }];
        let (mut candidate, _) = exclusion(&old, Action::AssetOnly);
        match group {
            0 => candidate.selected.0 = None,
            1 => candidate.selected.1 = None,
            _ => candidate.selected.2 = None,
        }
        assert_eq!(
            admit(&old, &candidate, &changes, NOW, false),
            Err(WriteError::Held)
        );
    }
}

#[test]
fn n32_none_selection_can_be_set_by_matching_change() {
    let mut old = fixture();
    old.selected = (None, None, None);
    for change in [
        cleanup(),
        Change::SetS1ChunkStrategy {
            strategy: reference("chunk-new"),
        },
        Change::SetDedupKeys {
            keys: reference("dedup-new"),
        },
    ] {
        let candidate = apply(&old, &resolved(&old, &change), from_ref(&change), NOW).unwrap();
        assert_eq!(admit(&old, &candidate, &[change], NOW, false), Ok(()));
        assert_ne!(candidate.selected, old.selected);
    }
}

#[test]
fn n32_per_source_precedence_is_resolved_by_caller() {
    let mut old = fixture();
    let mut source = old.policy.sources[0].clone();
    source.id = "second".into();
    old.policy.sources.push(source);
    old.sources
        .insert("second".into(), old.sources["notes"].clone());
    let mut input = resolved(&old, &cleanup());
    input.sources.get_mut("second").unwrap().2 = old.sources["second"].2.clone();
    let candidate = apply(&old, &input, &[cleanup()], NOW).unwrap();
    assert_eq!(
        candidate.sources["notes"].2.cleanup,
        reference("cleanup-new")
    );
    assert_eq!(
        candidate.sources["second"].2.cleanup,
        old.sources["second"].2.cleanup
    );
    assert_eq!(admit(&old, &candidate, &[cleanup()], NOW, false), Ok(()));
    let mut smuggled = candidate;
    smuggled.sources.get_mut("second").unwrap().2.dedup = reference("unapproved");
    assert_eq!(
        admit(&old, &smuggled, &[cleanup()], NOW, false),
        Err(WriteError::Held)
    );
}

#[test]
fn n32_empty_change_or_invalid_clock_holds() {
    let old = fixture();
    assert_eq!(
        apply(&old, &old, &[], NOW).map(|_| ()),
        Err(WriteError::Held)
    );
    assert_eq!(
        apply(&old, &old, &[cleanup()], "invalid").map(|_| ()),
        Err(WriteError::Held)
    );
}

#[test]
fn n32_processing_reference_must_match_exact_digest() {
    let old = fixture();
    let mut target = reference("cleanup-new");
    target.digest = reference("other").digest;
    assert_eq!(
        apply(&old, &old, &[Change::SetCleanup { rules: target }], NOW).map(|_| ()),
        Err(WriteError::Held)
    );
}

#[test]
fn n32_effective_processing_cannot_smuggle_unrelated_ids() {
    let old = fixture();
    for change in [
        cleanup(),
        Change::SetS1ChunkStrategy {
            strategy: reference("chunk-new"),
        },
        Change::SetDedupKeys {
            keys: reference("dedup-new"),
        },
    ] {
        let mut candidate = resolved(&old, &change);
        let effective = &mut candidate.sources.get_mut("notes").unwrap().2;
        match change {
            Change::SetCleanup { .. } => effective.cleanup = reference("unapproved"),
            Change::SetS1ChunkStrategy { .. } => effective.chunk = reference("unapproved"),
            _ => effective.dedup = reference("unapproved"),
        }
        assert_eq!(
            apply(&old, &candidate, &[change], NOW).map(|_| ()),
            Err(WriteError::Held)
        );
    }
}

#[test]
fn n32_profile_reference_state_and_resolution_must_match() {
    for fault in [
        "reference",
        "state",
        "resolved_reference",
        "resolved_definition",
        "source",
        "missing",
    ] {
        let mut old = fixture();
        let mut target = old.profiles["new"].reference();
        let mut input = resolved(
            &old,
            &Change::SelectProfile {
                source_id: "notes".into(),
                profile: target.clone(),
                selection: reference("selection-new"),
            },
        );
        let mut source_id = "notes".to_owned();
        match fault {
            "reference" => {
                target.digest = reference("wrong").digest;
                input.sources.get_mut("notes").unwrap().0 = target.clone();
            }
            "state" => {
                old.profiles
                    .get_mut("new")
                    .unwrap()
                    .definition
                    .qualification_state = QualificationState::Held;
                input
                    .sources
                    .get_mut("notes")
                    .unwrap()
                    .1
                    .qualification_state = QualificationState::Held;
            }
            "resolved_reference" => input.sources.get_mut("notes").unwrap().0 = reference("wrong"),
            "resolved_definition" => input.sources.get_mut("notes").unwrap().1.languages.clear(),
            "source" => source_id = "missing".into(),
            _ => {
                old.profiles.remove("new");
            }
        }
        assert_eq!(
            apply(
                &old,
                &input,
                &[Change::SelectProfile {
                    source_id,
                    profile: target,
                    selection: reference("selection-new")
                }],
                NOW
            )
            .map(|_| ()),
            Err(WriteError::Held),
            "{fault}"
        );
    }
}

#[test]
fn n32_selected_profile_cannot_import_unapproved_processing() {
    let old = fixture();
    let change = Change::SelectProfile {
        source_id: "notes".into(),
        profile: old.profiles["new"].reference(),
        selection: reference("selection-new"),
    };
    for group in ["cleanup", "chunk", "dedup"] {
        let mut candidate = resolved(&old, &change);
        let effective = &mut candidate.sources.get_mut("notes").unwrap().2;
        match group {
            "cleanup" => effective.cleanup = reference("unapproved"),
            "chunk" => effective.chunk = reference("unapproved"),
            _ => effective.dedup = reference("unapproved"),
        }
        assert_eq!(
            apply(&old, &candidate, from_ref(&change), NOW).map(|_| ()),
            Err(WriteError::Held),
            "{group}"
        );
    }
}

#[test]
fn n32_chunk_qualification_cannot_be_missing_or_zero() {
    for zero in [false, true] {
        let mut old = fixture();
        if zero {
            old.qualified_chunk_tokens.insert("chunk-new".into(), 0);
        } else {
            old.qualified_chunk_tokens.clear();
        }
        assert_eq!(
            apply(
                &old,
                &old,
                &[Change::SetS1ChunkStrategy {
                    strategy: reference("chunk-new")
                }],
                NOW
            )
            .map(|_| ()),
            Err(WriteError::Held)
        );
    }
}

#[test]
fn n32_exclusion_binding_selector_and_time_must_be_narrowing() {
    let old = fixture();
    for fault in [
        "reference",
        "id",
        "source",
        "selector",
        "future",
        "invalid_time",
        "invalid_expiry",
        "fractional_expiry",
    ] {
        let (mut candidate, change) = exclusion(&old, Action::AssetOnly);
        let (reference, entry) = candidate.decisions.get_mut("new-exclusion").unwrap();
        match fault {
            "reference" => reference.digest = super::n32_support::reference("other").digest,
            "id" => entry.id = "other".into(),
            "source" => entry.selector.source_id = "other".into(),
            "selector" => entry.selector.path_prefix = Some("/outside".into()),
            "future" => entry.effective_at = "2027-01-01T00:00:00Z".into(),
            "invalid_time" => entry.effective_at = "invalid".into(),
            "invalid_expiry" => entry.expires_at = Some("invalid".into()),
            _ => entry.expires_at = Some("2026-10-01T00:00:00.000Z".into()),
        }
        assert_eq!(
            apply(&old, &candidate, from_ref(&change), NOW).map(|_| ()),
            Err(WriteError::Held),
            "{fault}"
        );
        assert_eq!(
            admit(&old, &candidate, &[change], NOW, false),
            Err(WriteError::Held),
            "{fault}"
        );
    }
    let (mut candidate, change) = exclusion(&old, Action::AssetOnly);
    candidate
        .decisions
        .get_mut("new-exclusion")
        .unwrap()
        .1
        .expires_at = Some("2026-10-01T00:00:00.001Z".into());
    assert_eq!(admit(&old, &candidate, &[change], NOW, false), Ok(()));
}

#[test]
fn n32_chunk_approved_reference_cannot_select_cleanup() {
    let old = fixture();
    assert_eq!(
        apply(
            &old,
            &old,
            &[Change::SetCleanup {
                rules: reference("chunk-new")
            }],
            NOW
        )
        .map(|_| ()),
        Err(WriteError::Held)
    );
}
