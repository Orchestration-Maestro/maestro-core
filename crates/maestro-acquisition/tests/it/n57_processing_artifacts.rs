//! Scoped processing contracts use synthetic payloads only.
use super::{n30_support::Fixture, n57_support as support};
use maestro_acquisition::adaptation::change::selection_key;
use maestro_acquisition::{
    adaptation::{
        Change,
        artifacts::{CleanupAction, CleanupRule, CleanupRules},
        change::{admit, apply},
        snapshot::ProcessingSnapshot,
        storage,
    },
    extraction::{
        detect::{Observation, Structure},
        model::Processing,
        outcome::ProfileSelection,
    },
    policy::{resolve::parse_resource, resource::Visibility},
};
use maestro_kernel::artifact::Digest;
use serde_json::json;
pub(super) fn initial(fixture: &Fixture) -> ProcessingSnapshot {
    storage::artifact(
        &fixture.db,
        "synthetic-reader",
        &fixture.manifest.processing_baseline,
    )
    .unwrap()
}
pub(super) fn check(fixture: &Fixture, snapshot: &ProcessingSnapshot) -> bool {
    let pin = support::retain(&fixture.db, snapshot, &[]);
    let principal = super::support::principal(&fixture.scopes);
    support::reader(
        &fixture.db,
        &fixture.collection,
        &fixture.catalog,
        &principal,
    )
    .read(&pin)
    .is_ok()
}
#[test]
fn n57_processing_ref_goldens() {
    let pins = concat!(
        r#"{"cleanup":{"id":"cleanup","digest":"11111111111111111111111111111111111111111"#,
        r#"11111111111111111111111"},"chunk":{"id":"chunk","digest":"22222222222222222222"#,
        r#"22222222222222222222222222222222222222222222"},"dedup":{"id":"dedup","digest":"#,
        r#""3333333333333333333333333333333333333333333333333333333333333333"}}"#,
    )
    .as_bytes();
    let processing: Processing = parse_resource(pins).expect("digest-bound processing pins");
    assert_eq!(serde_json::to_vec(&processing).unwrap(), pins);
    assert!(parse_resource::<Processing>(br#"{"cleanup":[],"chunk":"chunk","dedup":[]}"#).is_err());
    assert_eq!(
        Digest::of(pins).as_str(),
        "794d6f84b660125515c7ec3006f8c47721e1a776e4eced9338814c584e942af1"
    );
}
#[test]
fn n57_strict_artifact_schemas() {
    let fixture = Fixture::new();
    let snapshot = initial(&fixture);
    let pin = &snapshot.effective.sources["notes"].2.cleanup;
    let mut rules: CleanupRules = storage::artifact(&fixture.db, "synthetic-reader", pin).unwrap();
    let rule = CleanupRule {
        id: "navigation".into(),
        selector: Structure::Observed {
            observation: Observation::Block {
                id: "navigation".into(),
            },
        },
        action: CleanupAction::OmitNavigation,
        reason_code: "chrome".into(),
    };
    rules.rules = vec![rule.clone()];
    let bytes = serde_json::to_vec(&rules).unwrap();
    assert!(parse_resource::<CleanupRules>(&bytes).is_ok());
    for case in [
        "unknown",
        "duplicate",
        "selector",
        "action",
        "duplicate-rule",
        "nodes",
        "deep",
        "invalid-id",
    ] {
        let mut value = json!(rules);
        match case {
            "unknown" => value["extra"] = json!(true),
            "duplicate" => {
                let text =
                    String::from_utf8(bytes.clone())
                        .unwrap()
                        .replacen('{', "{\"version\":1,", 1);
                assert!(parse_resource::<CleanupRules>(text.as_bytes()).is_err());
                continue;
            }
            "selector" => value["rules"][0]["selector"] = json!({"kind":"css","query":"*"}),
            "action" => value["rules"][0]["action"] = json!("rewrite"),
            "duplicate-rule" => value["rules"] = json!([rule, rule]),
            "nodes" => {
                value["rules"][0]["selector"] =
                    json!({"kind":"all","children":vec![rule.selector.clone();1000]});
            }
            "deep" => {
                for _ in 0..33 {
                    value["rules"][0]["selector"] =
                        json!({"kind":"all","children":[value["rules"][0]["selector"].clone()]});
                }
            }
            _ => value["rules"][0]["id"] = json!("../rule"),
        }
        assert!(
            parse_resource::<CleanupRules>(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{case}"
        );
    }
    let mut duplicate = rules.clone();
    duplicate.rules.push(rule);
    assert!(parse_resource::<CleanupRules>(&serde_json::to_vec(&duplicate).unwrap()).is_err());
    let mut effective = json!(snapshot);
    effective["effective"]["unexpected"] = json!(true);
    assert!(
        parse_resource::<ProcessingSnapshot>(&serde_json::to_vec(&effective).unwrap()).is_err()
    );
    let mut held = json!({"kind":"held","reason":"unknown"});
    assert!(parse_resource::<ProfileSelection>(&serde_json::to_vec(&held).unwrap()).is_err());
    held["reason"] = json!("ambiguous");
    assert!(parse_resource::<ProfileSelection>(&serde_json::to_vec(&held).unwrap()).is_err());
}
#[test]
fn n57_profile_defaults_and_collection_precedence() {
    let fixture = Fixture::new();
    let old = initial(&fixture);
    assert!(check(&fixture, &old));
    assert_ne!(
        old.effective.sources["notes"].2,
        old.effective.sources["second"].2
    );
    for group in 0..3 {
        let mut snapshot = old.clone();
        let second = snapshot.effective.sources["second"].2.clone();
        match group {
            0 => snapshot.effective.selected.0 = Some(second.cleanup.clone()),
            1 => snapshot.effective.selected.1 = Some(second.chunk.clone()),
            _ => snapshot.effective.selected.2 = Some(second.dedup.clone()),
        }
        assert!(
            !check(&fixture, &snapshot),
            "stored precedence claim group {group}"
        );
        for (_, _, processing) in snapshot.effective.sources.values_mut() {
            match group {
                0 => processing.cleanup = second.cleanup.clone(),
                1 => processing.chunk = second.chunk.clone(),
                _ => processing.dedup = second.dedup.clone(),
            }
        }
        assert!(
            check(&fixture, &snapshot),
            "collection override group {group}"
        );
    }
}
#[test]
fn n57_later_profile_change_respects_null_and_override_slots() {
    let fixture = Fixture::new();
    let old = initial(&fixture);
    let mut selected = old.clone();
    let profile = selected.effective.profiles["novel"].clone();
    let evidence = support::retain(
        &fixture.db,
        &json!({"synthetic":"later source sample"}),
        &[],
    );
    let pin = support::retain(
        &fixture.db,
        &ProfileSelection::Selected {
            profile: profile.reference(),
            evidence: vec![evidence.clone()],
        },
        &[evidence.id.parse().unwrap()],
    );
    selected
        .effective
        .protected_resources
        .insert(selection_key("notes"), pin.clone());
    selected.effective.policy.sources[0].selected_profiles = vec![profile.reference()];
    selected.effective.sources.insert(
        "notes".into(),
        (
            profile.reference(),
            profile.definition.clone(),
            profile.definition.processing.clone(),
        ),
    );
    assert!(check(&fixture, &selected));
    let change = Change::SelectProfile {
        source_id: "notes".into(),
        profile: profile.reference(),
        selection: pin,
    };
    assert!(
        admit(
            &old.effective,
            &selected.effective,
            &[change],
            super::n32_support::NOW,
            false
        )
        .is_ok()
    );
    selected.effective.selected.0 = Some(old.effective.sources["notes"].2.cleanup.clone());
    assert!(!check(&fixture, &selected));
    for (_, _, processing) in selected.effective.sources.values_mut() {
        processing.cleanup = old.effective.sources["notes"].2.cleanup.clone();
    }
    assert!(check(&fixture, &selected));
    let mut cleared = selected.effective.clone();
    cleared.selected.0 = None;
    let changes = [Change::SetCleanup {
        rules: old.effective.sources["notes"].2.cleanup.clone(),
    }];
    assert!(
        admit(
            &selected.effective,
            &cleared,
            &changes,
            super::n32_support::NOW,
            false
        )
        .is_err()
    );
    assert!(apply(&old.effective, &old.effective, &[], super::n32_support::NOW).is_err());
}
#[test]
fn n57_scoped_snapshot_and_candidate_binding() {
    let fixture = Fixture::new();
    let principal = super::support::principal(&fixture.scopes);
    let reader = support::reader(
        &fixture.db,
        &fixture.collection,
        &fixture.catalog,
        &principal,
    );
    assert!(reader.candidate(&fixture.proposal).is_ok());
    for fault in [
        "arbitrary",
        "missing",
        "duplicate",
        "noncanonical",
        "baseline",
    ] {
        let mut proposal = fixture.proposal.clone();
        match fault {
            "arbitrary" => proposal.candidate = Digest::of(b"arbitrary"),
            "missing" => proposal.evidence.pop().map(|_| ()).unwrap(),
            "duplicate" => proposal.evidence.push(*proposal.evidence.last().unwrap()),
            "noncanonical" => {
                let snapshot = initial(&fixture);
                let bytes = serde_json::to_string_pretty(&snapshot).unwrap();
                let pin =
                    storage::retain(&fixture.db, &[support::TAG.into()], bytes.as_bytes(), &[])
                        .unwrap();
                proposal.candidate = pin.digest;
                proposal.evidence = vec![pin.id.parse().unwrap()];
            }
            _ => proposal.expected_baseline.digest = Digest::of(b"other baseline"),
        }
        assert!(reader.candidate(&proposal).is_err(), "{fault}");
    }
}
#[test]
fn n57_scoped_snapshot_claims_are_revalidated() {
    let fixture = Fixture::new();
    let old = initial(&fixture);
    for fault in [
        "collection",
        "baseline-pin",
        "owner",
        "visibility",
        "scope",
        "definition",
        "wrong-class",
        "map-key",
        "qualified-limit",
        "missing",
        "substituted",
        "profile",
        "non-selection-policy",
        "added-profile",
    ] {
        let mut snapshot = old.clone();
        match fault {
            "baseline-pin" => {
                snapshot.effective.baseline.digest = Digest::of(b"wrong baseline pin");
            }
            "collection" => snapshot.resource.collection_id = "other".into(),
            "owner" => snapshot.resource.owner_ref.digest = Digest::of(b"wrong owner"),
            "visibility" => {
                snapshot.resource.visibility = Visibility::Private;
            }
            "scope" => snapshot
                .resource
                .scope_tags
                .push("workspace/default/collection/other".into()),
            "definition" => snapshot
                .effective
                .sources
                .get_mut("notes")
                .unwrap()
                .1
                .languages
                .clear(),
            "wrong-class" => {
                let pin = snapshot.effective.sources["notes"].2.chunk.clone();
                snapshot
                    .effective
                    .approved_cleanup
                    .insert(pin.id.clone(), pin);
            }
            "map-key" => {
                let pin = snapshot
                    .effective
                    .approved_cleanup
                    .values()
                    .next()
                    .unwrap()
                    .clone();
                snapshot
                    .effective
                    .approved_cleanup
                    .insert("wrong-key".into(), pin);
            }
            "qualified-limit" => snapshot.effective.qualified_model_limit += 1,
            "missing" => snapshot.effective.approved_cleanup.clear(),
            "substituted" => {
                snapshot
                    .effective
                    .approved_cleanup
                    .values_mut()
                    .next()
                    .unwrap()
                    .digest = Digest::of(b"substitution");
            }
            "profile" => snapshot
                .effective
                .profiles
                .get_mut("markdown")
                .unwrap()
                .definition
                .languages
                .clear(),
            "non-selection-policy" => {
                snapshot.effective.policy.aggregate_limits.pages =
                    (old.effective.policy.aggregate_limits.pages.get() + 1)
                        .try_into()
                        .unwrap();
            }
            _ => {
                let pin = snapshot.effective.profiles["safe"].reference();
                snapshot.effective.policy.sources[0]
                    .selected_profiles
                    .push(pin);
            }
        }
        assert!(!check(&fixture, &snapshot), "{fault}");
    }
}

#[test]
fn n57_effective_object_shapes_are_strict() {
    let fixture = Fixture::new();
    let mut value = json!(initial(&fixture));
    let processing = value["effective"]["sources"]["notes"][2].clone();
    value["effective"]["sources"]["notes"][2] = json!([
        processing["cleanup"],
        processing["chunk"],
        processing["dedup"]
    ]);
    assert!(parse_resource::<ProcessingSnapshot>(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn n57_retained_snapshots_must_roundtrip_strict_wire_shape() {
    let fixture = Fixture::new();
    let principal = super::support::principal(&fixture.scopes);
    let reader = support::reader(
        &fixture.db,
        &fixture.collection,
        &fixture.catalog,
        &principal,
    );
    let mut snapshot = initial(&fixture);
    assert!(reader.retain(&snapshot).is_ok());
    snapshot.resource.id = "../invalid".into();
    assert!(reader.retain(&snapshot).is_err());
}
