//! Pure synthetic N32 controls; runtime artifact contracts belong to N34.
use super::n32_support::{
    NOW, cleanup, disjoint_exclusion, disjoint_fixture, exclusion, fixture, held, reference,
    resolved,
};
use maestro_acquisition::{
    adaptation::{
        Change, WriteError,
        change::{admit, apply},
    },
    extraction::registry::definition_bytes,
    policy::{decisions::Action, manifest::AutomaticClass, schema::SourcePolicy},
};
use maestro_kernel::artifact::Digest;
use std::slice::from_ref;

#[test]
fn n32_five_classes_and_both_narrowing_actions_are_allowed() {
    let mut old = fixture();
    for (change, class) in [
        (
            Change::SelectProfile {
                source_id: "notes".into(),
                profile: old.profiles["new"].reference(),
                selection: reference("selection-new"),
            },
            AutomaticClass::SelectedProfiles,
        ),
        (cleanup(), AutomaticClass::Cleanup),
        (
            Change::SetS1ChunkStrategy {
                strategy: reference("chunk-new"),
            },
            AutomaticClass::S1ChunkStrategy,
        ),
        (
            Change::SetDedupKeys {
                keys: reference("dedup-new"),
            },
            AutomaticClass::DedupKeys,
        ),
    ] {
        old.policy.adaptation.automatic_classes = vec![class];
        let candidate = apply(&old, &resolved(&old, &change), from_ref(&change), NOW);
        assert_eq!(candidate.as_ref().map(|_| ()), Ok(()), "{class:?}");
        let candidate = candidate.unwrap();
        assert_eq!(
            admit(&old, &candidate, from_ref(&change), NOW, false),
            Ok(())
        );
        assert_ne!(candidate, old);
        old.policy.adaptation.automatic_classes = vec![AutomaticClass::NewKnowledgeExclusions];
        assert_eq!(
            apply(&old, &resolved(&old, &change), &[change], NOW).map(|_| ()),
            Err(WriteError::Held)
        );
    }
    let mut old = disjoint_fixture();
    for action in [Action::ExcludeFromKnowledge, Action::AssetOnly] {
        old.policy.adaptation.automatic_classes = vec![AutomaticClass::NewKnowledgeExclusions];
        let (candidate, change) = disjoint_exclusion(&old, action);
        assert_eq!(admit(&old, &candidate, &[change], NOW, false), Ok(()));
    }
}

#[test]
fn n32_denies_identity_binding_and_baseline() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate.baseline = reference("other");
    held(&old, &candidate, &[cleanup()]);
    candidate = old.clone();
    candidate.policy.resource.collection_id = "other".into();
    held(&old, &candidate, &[cleanup()]);
}

#[test]
fn n32_denies_scopes_crawl_and_cadence() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate.policy.sources[0]
        .seeds
        .push("https://garden.example/outside".into());
    held(&old, &candidate, &[cleanup()]);
    candidate = old.clone();
    candidate.policy.sources[0].sync.overlap_ms += 1;
    held(&old, &candidate, &[cleanup()]);
}

#[test]
fn n32_denies_existing_exclusion_edit_removal_and_expiry() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate.decisions.clear();
    held(&old, &candidate, &[cleanup()]);
    for field in ["action", "expiry", "selector"] {
        candidate = old.clone();
        let entry = &mut candidate.decisions.get_mut("no-assets").unwrap().1;
        match field {
            "action" => entry.action = Action::ExcludeFromKnowledge,
            "expiry" => entry.expires_at = Some(NOW.into()),
            _ => entry.selector.path_prefix = Some("/other".into()),
        }
        held(&old, &candidate, &[cleanup()]);
    }
}

#[test]
fn n32_existing_promotions_cannot_be_changed() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate.policy.sources[0]
        .promotions
        .push(reference("promote"));
    held(&old, &candidate, &[cleanup()]);
}

#[test]
fn n32_denies_robots_and_overrides() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate.policy.sources[0].robots.r#override = Some(reference("override"));
    held(&old, &candidate, &[cleanup()]);
}

#[test]
fn n32_denies_rates_and_budgets() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate.policy.aggregate_limits.pages = 2.try_into().unwrap();
    held(&old, &candidate, &[cleanup()]);
}

#[test]
fn n32_denies_url_and_query_identity() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate.policy.sources[0]
        .identity
        .ignored_tracking_queries
        .push("meaningful".into());
    held(&old, &candidate, &[cleanup()]);
}

#[test]
fn n32_denies_network_authentication_and_grants() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate.policy.sources[0].auth_role = Some("new-role".into());
    held(&old, &candidate, &[cleanup()]);
    candidate = old.clone();
    candidate.policy.sources[0].origins[0].private_grant = Some("new-grant".into());
    held(&old, &candidate, &[cleanup()]);
}

#[test]
fn n32_denies_transport_adapter_and_readiness() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate.policy.sources[0].acquisition_profile = reference("browser");
    held(&old, &candidate, &[cleanup()]);
    candidate = old.clone();
    candidate
        .protected_resources
        .insert("readiness".into(), reference("new-readiness"));
    held(&old, &candidate, &[cleanup()]);
}

#[test]
fn n32_denies_profile_definitions_and_fidelity_tolerances() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate
        .profiles
        .get_mut("old")
        .unwrap()
        .definition
        .required_fidelity
        .clear();
    held(&old, &candidate, &[cleanup()]);
}

#[test]
fn n32_denies_wiki_and_permission_mappings() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate
        .protected_resources
        .insert("wiki-permissions".into(), reference("wide"));
    held(&old, &candidate, &[cleanup()]);
}

#[test]
fn n32_denies_warn_only_admission() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate
        .profiles
        .get_mut("old")
        .unwrap()
        .definition
        .admission_rules = vec!["warn-only".into()];
    held(&old, &candidate, &[cleanup()]);
}

#[test]
fn n32_denies_matrix_gold_threshold_drift_and_activation_policy() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate.policy.adaptation.thresholds = reference("loose");
    held(&old, &candidate, &[cleanup()]);
}

#[test]
fn n32_denies_resource_retention_and_deletion() {
    let old = fixture();
    let mut candidate = old.clone();
    candidate.policy.retention_rule = reference("delete");
    held(&old, &candidate, &[cleanup()]);
}

#[test]
fn n32_unknown_fields_and_change_groups_refuse() {
    let old = fixture();
    let bytes = serde_json::to_vec(&old.policy).unwrap();
    let mut text = String::from_utf8(bytes).unwrap();
    text.insert_str(1, "\"unknown\":true,");
    assert!(serde_json::from_str::<SourcePolicy>(&text).is_err());
    for group in [
        "identity",
        "scopes",
        "existing_exclusions",
        "promotions",
        "robots",
        "budgets",
        "url_identity",
        "grants",
        "transport",
        "profile_definition",
        "wiki",
        "warn_only",
        "matrix",
        "retention",
        "unknown",
    ] {
        assert!(
            serde_json::from_str::<Change>(&format!("{{\"kind\":\"{group}\"}}")).is_err(),
            "{group}"
        );
    }
    let mut change = String::from_utf8(serde_json::to_vec(&cleanup()).unwrap()).unwrap();
    change.insert_str(1, "\"unknown\":true,");
    assert!(serde_json::from_str::<Change>(&change).is_err());
}

#[test]
fn n32_indirect_profile_substitution_holds_even_when_qualified() {
    let mut old = fixture();
    old.profiles
        .get_mut("new")
        .unwrap()
        .definition
        .admission_rules = vec!["warn-only".into()];
    let new = old.profiles.get_mut("new").unwrap();
    new.definition_digest = Digest::of(&definition_bytes(&new.definition).unwrap());
    let change = Change::SelectProfile {
        source_id: "notes".into(),
        profile: new.reference(),
        selection: reference("selection-new"),
    };
    assert_eq!(
        apply(&old, &resolved(&old, &change), &[change], NOW),
        Err(WriteError::Held)
    );
}

#[test]
fn n32_mixed_proposal_is_held_whole_despite_passing_scores() {
    let old = fixture();
    let changes = [
        cleanup(),
        Change::SetDedupKeys {
            keys: reference("dedup-new"),
        },
    ];
    let input = resolved(&resolved(&old, &changes[0]), &changes[1]);
    let mut candidate = apply(&old, &input, &changes, NOW).unwrap();
    candidate.policy.sources[0].auth_role = Some("unauthorized".into());
    held(&old, &candidate, &changes);
    assert_eq!(old.sources["notes"].2.cleanup, reference("cleanup-default"));
}

#[test]
fn n32_expired_exclusion_never_readmits() {
    let old = fixture();
    let (mut candidate, change) = exclusion(&old, Action::AssetOnly);
    candidate
        .decisions
        .get_mut("new-exclusion")
        .unwrap()
        .1
        .expires_at = Some(NOW.into());
    held(&old, &candidate, &[change]);
    // Removing a previously activated, now expired entry also holds.
    let active = candidate;
    let mut readmit = active.clone();
    readmit.decisions.remove("new-exclusion");
    held(&active, &readmit, &[cleanup()]);
}

#[test]
fn n32_new_exclusion_must_only_narrow() {
    let old = fixture();
    let (candidate, _) = exclusion(&old, Action::DenyFetch);
    held(
        &old,
        &candidate,
        &[Change::AddAssetOnly {
            entry: reference("new-exclusion"),
        }],
    );
    let (_, mut change) = exclusion(&old, Action::AssetOnly);
    if let Change::AddAssetOnly { entry } = &mut change {
        *entry = reference("no-assets");
    }
    held(&old, &old, &[change]);
}

#[test]
fn n32_unauthorized_not_applicable_holds() {
    let old = fixture();
    let changes = [cleanup()];
    let candidate = apply(&old, &old, &changes, NOW).unwrap();
    assert_eq!(
        admit(&old, &candidate, &changes, NOW, true),
        Err(WriteError::Held)
    );
}

#[test]
fn n32_chunk_strategy_must_fit_qualified_model_limit() {
    let mut old = fixture();
    old.qualified_model_limit = 699;
    assert_eq!(
        apply(
            &old,
            &old,
            &[Change::SetS1ChunkStrategy {
                strategy: reference("chunk-new")
            }],
            NOW
        ),
        Err(WriteError::Held)
    );
}

#[test]
fn n32_disabled_classes_and_unresolved_selections_hold() {
    let mut old = fixture();
    old.policy.adaptation.automatic_classes.clear();
    assert_eq!(apply(&old, &old, &[cleanup()], NOW), Err(WriteError::Held));
    old = fixture();
    old.approved_cleanup.clear();
    assert_eq!(apply(&old, &old, &[cleanup()], NOW), Err(WriteError::Held));
}
