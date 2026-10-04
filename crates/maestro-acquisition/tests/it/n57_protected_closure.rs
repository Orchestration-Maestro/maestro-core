//! Protected identities come from current validation, not deserialized claims.
use super::{
    n30_support::Fixture,
    n57_processing_artifacts::{check, initial},
    support,
};
use maestro_acquisition::{ImmutableResource, Principal, Ref, Refusal, ResourceSource, validate};
use serde_json::json;
use std::{collections::BTreeMap, sync::Mutex};

/// Records the validator's real reads without manufacturing admission.
#[derive(Debug)]
struct Recording<'a> {
    source: &'a support::Catalog,
    reads: Mutex<BTreeMap<String, Ref>>,
}
impl ResourceSource for Recording<'_> {
    fn read(
        &self,
        reference: &Ref,
        principal: &Principal<'_>,
    ) -> Result<ImmutableResource, Refusal> {
        self.reads
            .lock()
            .unwrap()
            .insert(reference.id.clone(), reference.clone());
        self.source.read(reference, principal)
    }
}
#[test]
fn n57_checked_references_equal_sorted_resolved_closure() {
    let fixture = Fixture::new();
    let source = Recording {
        source: &fixture.catalog,
        reads: Mutex::new(BTreeMap::new()),
    };
    let checked = validate(
        &source,
        &fixture.collection,
        &support::principal(&fixture.scopes),
    )
    .unwrap();
    let expected: Vec<_> = source.reads.lock().unwrap().values().cloned().collect();
    assert_eq!(checked.references(), expected);
    assert!(
        checked
            .references()
            .windows(2)
            .all(|pair| pair.first().unwrap().id < pair.last().unwrap().id)
    );
}
fn protected_fault(fault: &str) {
    let fixture = Fixture::new();
    let mut snapshot = initial(&fixture);
    assert!(check(&fixture, &snapshot));
    let key = fixture.manifest.baseline.id.clone();
    match fault {
        "missing" => {
            snapshot.effective.protected_resources.remove(&key);
        }
        "extra" => {
            snapshot
                .effective
                .protected_resources
                .insert("extra".into(), fixture.manifest.baseline.clone());
        }
        _ => {
            snapshot.effective.protected_resources.insert(
                key,
                fixture.catalog.0.get("owner").unwrap().reference.clone(),
            );
        }
    }
    assert!(!check(&fixture, &snapshot), "{fault}");
}
#[test]
fn n57_missing_protected_entry_holds_requalification() {
    protected_fault("missing");
}
#[test]
fn n57_extra_protected_entry_holds_requalification() {
    protected_fault("extra");
}
#[test]
fn n57_swapped_protected_ref_holds_requalification() {
    protected_fault("swapped");
}

#[test]
fn n57_baseline_ids_cannot_shadow_typed_protected_namespaces() {
    for id in ["profile_selection.extra", "s1_embedding_model"] {
        let mut fixture = Fixture::new();
        let mut snapshot = initial(&fixture);
        let reference = support::put(&mut fixture.catalog, id, &json!({"synthetic":"override"}));
        let mut policy = support::value(&fixture.catalog, "policy");
        policy["sources"][0]["robots"]["override"] = json!(reference);
        support::put(&mut fixture.catalog, "policy", &policy);
        let mut collection = serde_json::to_value(&fixture.collection).unwrap();
        support::rebind(&mut collection, &mut fixture.catalog);
        fixture.collection = serde_json::from_value(collection).unwrap();
        let checked = validate(
            &fixture.catalog,
            &fixture.collection,
            &support::principal(&fixture.scopes),
        )
        .unwrap();
        snapshot.effective.baseline = checked.reference().clone();
        snapshot.effective.policy = checked.policy().clone();
        snapshot.effective.protected_resources.retain(|key, _| {
            key.starts_with("profile_selection.")
                || key == "s1_embedding_model"
                || key == "s1_tokenizer_qualification"
        });
        for reference in checked.references() {
            if reference.id != "s1_embedding_model" {
                snapshot
                    .effective
                    .protected_resources
                    .insert(reference.id.clone(), reference.clone());
            }
        }
        assert!(!check(&fixture, &snapshot), "{id}");
    }
}
