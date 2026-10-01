//! New exclusions use N03's exact singleton artifact, not a parallel schema.
use super::{
    n30_support::Fixture,
    n57_processing_artifacts::{check, initial},
    n57_support as support,
};
use maestro_acquisition::{
    AdmissionStatus, Ref,
    adaptation::{Change, change::admit},
    policy::decisions::{Action, DecisionSchema, Decisions},
};
use maestro_kernel::artifact::Digest;
use serde_json::json;

#[test]
fn n57_singleton_decisions_require_closed_narrowing_actions() {
    for action in [
        Action::AssetOnly,
        Action::ExcludeFromKnowledge,
        Action::DenyFetch,
    ] {
        let mut fixture = Fixture::new();
        let mut snapshot = initial(&fixture);
        let mut entry = snapshot
            .effective
            .decisions
            .values()
            .next()
            .unwrap()
            .1
            .clone();
        entry.id = "new-decision".into();
        entry.action = action;
        let singleton = Decisions {
            resource: support::resource(
                DecisionSchema::V1,
                "singleton",
                &snapshot.effective.policy,
            ),
            entries: vec![entry.clone()],
            qualification: fixture.catalog.0["processing-qualification"]
                .reference
                .clone(),
        };
        let pin = support::retain(&fixture.db, &singleton, &[]);
        fixture
            .catalog
            .0
            .get_mut("processing-qualification")
            .unwrap()
            .admission
            .references
            .push(pin.clone());
        snapshot
            .effective
            .decisions
            .insert(entry.id.clone(), (pin, entry));
        assert_eq!(check(&fixture, &snapshot), action != Action::DenyFetch);
    }
}
#[test]
fn n57_singleton_count_and_existing_decision_identity_are_fixed() {
    let mut fixture = Fixture::new();
    let snapshot = initial(&fixture);
    let mut entry = snapshot
        .effective
        .decisions
        .values()
        .next()
        .unwrap()
        .1
        .clone();
    let original_id = entry.id.clone();
    entry.id = "new-decision".into();
    entry.action = Action::AssetOnly;
    for fault in [
        "count",
        "existing",
        "id",
        "removed",
        "empty-evidence",
        "at-evidence-limit",
        "large-evidence",
    ] {
        let mut snapshot = snapshot.clone();
        let mut entry = entry.clone();
        if fault == "empty-evidence" {
            entry.evidence.clear();
        }
        if fault == "at-evidence-limit" || fault == "large-evidence" {
            let count = if fault == "at-evidence-limit" {
                1000
            } else {
                1001
            };
            entry.evidence = vec![entry.evidence.first().unwrap().clone(); count];
        }
        let singleton = Decisions {
            resource: support::resource(
                DecisionSchema::V1,
                "singleton",
                &snapshot.effective.policy,
            ),
            entries: if fault == "count" {
                vec![entry.clone(), entry.clone()]
            } else {
                vec![entry.clone()]
            },
            qualification: fixture.catalog.0["processing-qualification"]
                .reference
                .clone(),
        };
        let pin = support::retain(&fixture.db, &singleton, &[]);
        fixture
            .catalog
            .0
            .get_mut("processing-qualification")
            .unwrap()
            .admission
            .references
            .push(pin.clone());
        match fault {
            "removed" => {
                snapshot.effective.decisions.clear();
            }
            "existing" => {
                snapshot
                    .effective
                    .decisions
                    .get_mut(&original_id)
                    .unwrap()
                    .0 = pin;
            }
            "id" => {
                snapshot
                    .effective
                    .decisions
                    .insert("wrong-id".into(), (pin, entry.clone()));
            }
            _ => {
                snapshot
                    .effective
                    .decisions
                    .insert(entry.id.clone(), (pin, entry.clone()));
            }
        }
        assert_eq!(
            check(&fixture, &snapshot),
            fault == "at-evidence-limit",
            "{fault}"
        );
    }
}

#[test]
fn n57_review_singleton_reversal_closure() {
    let mut observed = Vec::new();
    for fault in ["valid", "missing", "digest", "status", "platform"] {
        let mut fixture = Fixture::new();
        let old = initial(&fixture);
        let mut snapshot = old.clone();
        let mut reversal = super::support::put(
            &mut fixture.catalog,
            "review-reversal",
            &json!({"synthetic":"reversal"}),
        );
        match fault {
            "missing" => {
                reversal = Ref {
                    id: "missing-reversal".into(),
                    digest: Digest::of(b"missing"),
                }
            }
            "digest" => reversal.digest = Digest::of(b"wrong reversal bytes"),
            "status" => {
                let value = fixture.catalog.0.get_mut("review-reversal").unwrap();
                value.admission.status = AdmissionStatus::Revoked;
            }
            "platform" => {
                let value = fixture.catalog.0.get_mut("review-reversal").unwrap();
                value.admission.platform = "other-platform".into();
            }
            _ => {}
        }
        let mut entry = snapshot
            .effective
            .decisions
            .values()
            .next()
            .unwrap()
            .1
            .clone();
        entry.id = "review-decision".into();
        entry.action = Action::AssetOnly;
        entry.reversal = Some(reversal);
        let singleton = Decisions {
            resource: support::resource(
                DecisionSchema::V1,
                "review-singleton",
                &snapshot.effective.policy,
            ),
            entries: vec![entry.clone()],
            qualification: fixture.catalog.0["processing-qualification"]
                .reference
                .clone(),
        };
        let pin = support::retain(&fixture.db, &singleton, &[]);
        fixture
            .catalog
            .0
            .get_mut("processing-qualification")
            .unwrap()
            .admission
            .references
            .push(pin.clone());
        snapshot
            .effective
            .decisions
            .insert(entry.id.clone(), (pin.clone(), entry));
        let snapshot_pin = support::retain(&fixture.db, &snapshot, &[]);
        let principal = super::support::principal(&fixture.scopes);
        let reader = support::reader(
            &fixture.db,
            &fixture.collection,
            &fixture.catalog,
            &principal,
        );
        let resolved = reader.read(&snapshot_pin);
        let read_ok = resolved.is_ok();
        let admit_ok = resolved.is_ok_and(|resolved| {
            admit(
                &old.effective,
                &resolved.snapshot.effective,
                &[Change::AddAssetOnly { entry: pin }],
                super::n32_support::NOW,
                false,
            )
            .is_ok()
        });
        println!("R1 fault={fault} read_ok={read_ok} pure_admit_ok={admit_ok}");
        observed.push((fault, read_ok, admit_ok));
    }
    assert_eq!(
        observed,
        vec![
            ("valid", true, true),
            ("missing", false, false),
            ("digest", false, false),
            ("status", false, false),
            ("platform", false, false)
        ],
        "singleton reversal closure"
    );
}
