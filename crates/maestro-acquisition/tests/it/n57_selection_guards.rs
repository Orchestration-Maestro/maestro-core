//! Stored selection evidence is mandatory even for a single eligible profile.
use super::{
    n30_support::Fixture,
    n57_processing_artifacts::{check, initial},
    n57_support as support,
};
use maestro_acquisition::AdmissionStatus;
use maestro_acquisition::{
    adaptation::{change::selection_key, storage},
    extraction::outcome::ProfileSelection,
};
use maestro_kernel::artifact::Digest;
use serde_json::json;

fn selection_fault(fault: &str) {
    let fixture = Fixture::new();
    let mut snapshot = initial(&fixture);
    assert!(check(&fixture, &snapshot));
    let key = selection_key("notes");
    let original = snapshot.effective.protected_resources.get(&key).unwrap();
    let selection: ProfileSelection =
        storage::artifact(&fixture.db, "synthetic-reader", original).unwrap();
    let (mut profile, mut evidence) = match selection {
        ProfileSelection::Selected { profile, evidence } => Some((profile, evidence)),
        ProfileSelection::Held { .. } => None,
    }
    .unwrap();
    match fault {
        "missing" => {
            snapshot.effective.protected_resources.remove(&key);
        }
        "extra" => {
            snapshot
                .effective
                .protected_resources
                .insert(selection_key("extra"), original.clone());
        }
        "empty" => {
            evidence.clear();
        }
        "large" => {
            evidence = vec![evidence.first().unwrap().clone(); 1001];
        }
        "mismatch" => {
            profile = snapshot
                .effective
                .profiles
                .get("novel")
                .unwrap()
                .reference();
        }
        "evidence" => {
            evidence.first_mut().unwrap().digest = Digest::of(b"changed source sample");
        }
        _ => {}
    }
    if ["empty", "large", "mismatch", "evidence"].contains(&fault) {
        let selection = support::retain(
            &fixture.db,
            &ProfileSelection::Selected { profile, evidence },
            &[],
        );
        snapshot
            .effective
            .protected_resources
            .insert(key, selection);
    }
    assert!(!check(&fixture, &snapshot), "{fault}");
}
#[test]
fn n57_missing_selection_pin_holds() {
    selection_fault("missing");
}
#[test]
fn n57_extra_selection_pin_holds() {
    selection_fault("extra");
}
#[test]
fn n57_empty_selection_evidence_holds() {
    selection_fault("empty");
}
#[test]
fn n57_selection_ref_mismatch_holds() {
    selection_fault("mismatch");
}
#[test]
fn n57_selection_evidence_digest_mismatch_holds() {
    selection_fault("evidence");
}
#[test]
fn n57_ambiguous_or_held_selection_receipt_holds() {
    let fixture = Fixture::new();
    let mut snapshot = initial(&fixture);
    for payload in [
        json!({"kind":"held","reason":"ambiguous"}),
        json!({"kind":"held","reason":"unknown"}),
        json!([{"kind":"selected"},{"kind":"selected"}]),
    ] {
        let pin = support::retain(&fixture.db, &payload, &[]);
        snapshot
            .effective
            .protected_resources
            .insert(selection_key("notes"), pin);
        assert!(!check(&fixture, &snapshot));
    }
}
#[test]
fn n57_artifact_absent_from_qualification_holds() {
    let fixture = Fixture::new();
    let mut catalog = fixture.catalog.clone();
    catalog
        .0
        .get_mut("processing-qualification")
        .unwrap()
        .admission
        .references
        .clear();
    let principal = super::support::principal(&fixture.scopes);
    assert!(
        support::reader(&fixture.db, &fixture.collection, &catalog, &principal)
            .read(&fixture.manifest.processing_baseline)
            .is_err()
    );
}
#[test]
fn n57_long_source_ids_have_bounded_noncolliding_selection_keys() {
    assert_eq!(
        selection_key("notes"),
        "profile_selection.ab5aa97074c454a0632057e704220d9a6678fbf773a0a5806fc09b8173b07309"
    );
    let id = "x".repeat(128);
    assert_eq!(selection_key(&id).len(), 82);
    assert_ne!(selection_key(&id), selection_key("notes"));
}

#[test]
fn n57_selection_evidence_ceiling_holds() {
    selection_fault("large");
}
#[test]
fn n57_external_qualification_claims_are_current() {
    let fixture = Fixture::new();
    let principal = super::support::principal(&fixture.scopes);
    for fault in [
        "reference",
        "bytes",
        "admission-digest",
        "status",
        "platform",
    ] {
        let mut catalog = fixture.catalog.clone();
        let qualification = catalog.0.get_mut("processing-qualification").unwrap();
        match fault {
            "reference" => qualification.reference.digest = Digest::of(b"different reference"),
            "bytes" => qualification.bytes = b"different bytes".to_vec(),
            "admission-digest" => {
                qualification.admission.digest = Digest::of(b"different admission");
            }
            "status" => qualification.admission.status = AdmissionStatus::Proposed,
            _ => qualification.admission.platform = "different-platform".into(),
        }
        assert!(
            support::reader(&fixture.db, &fixture.collection, &catalog, &principal)
                .read(&fixture.manifest.processing_baseline)
                .is_err(),
            "{fault}"
        );
    }
}

#[test]
fn s6t_selection_exact_evidence_ceiling_is_admitted() {
    let fixture = Fixture::new();
    let mut snapshot = initial(&fixture);
    let key = selection_key("notes");
    let original = snapshot.effective.protected_resources.get(&key).unwrap();
    let selection: ProfileSelection =
        storage::artifact(&fixture.db, "synthetic-reader", original).unwrap();
    let ProfileSelection::Selected { profile, evidence } = selection else {
        panic!("selected fixture")
    };
    let evidence = vec![evidence.first().unwrap().clone(); 1000];
    let selection = support::retain(
        &fixture.db,
        &ProfileSelection::Selected { profile, evidence },
        &[],
    );
    snapshot
        .effective
        .protected_resources
        .insert(key, selection);
    assert!(check(&fixture, &snapshot));
}

#[test]
fn s6t_snapshot_qualified_pin_still_requires_source_eligibility() {
    let fixture = Fixture::new();
    let mut snapshot = initial(&fixture);
    snapshot
        .effective
        .policy
        .sources
        .first_mut()
        .unwrap()
        .selected_profiles
        .clear();
    assert!(!check(&fixture, &snapshot));
}
