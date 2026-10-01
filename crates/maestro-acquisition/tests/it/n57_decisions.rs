//! New exclusions use N03's exact singleton artifact, not a parallel schema.
use super::{
    n30_support::Fixture,
    n57_processing_artifacts::{check, initial},
    n57_support as support,
};
use maestro_acquisition::policy::decisions::{Action, DecisionSchema, Decisions};

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
    for fault in ["count", "existing", "id", "removed", "empty-evidence"] {
        let mut snapshot = snapshot.clone();
        let mut entry = entry.clone();
        if fault == "empty-evidence" {
            entry.evidence.clear();
        }
        if fault == "large-evidence" {
            entry.evidence = vec![entry.evidence.first().unwrap().clone(); 1001];
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
        assert!(!check(&fixture, &snapshot), "{fault}");
    }
}
