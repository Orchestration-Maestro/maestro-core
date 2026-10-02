//! Shared admission refuses authenticated sources before authority.
use super::flow_edges::{Fixture, clean};

#[test]
fn n14_shared_controls_refuse_authenticated_source_before_authority() {
    use super::{
        controls::Controls,
        controls::request,
        flow_tests::{Grants, fixture_with},
    };
    use maestro_acquisition::{
        Principal,
        policy::{decision::AdmissionControls, resolve::validate},
    };
    use std::env;
    let fixture = Fixture::new(clean);
    let scopes = fixture.db.visible("reader").unwrap();
    let principal = Principal {
        id: "reader",
        platform: env::consts::OS,
        scopes: &scopes,
    };
    let (collection, files) = fixture_with(|policy| {
        policy["sources"][0]["auth_role"] = serde_json::json!("synthetic-account");
    });
    let policy = validate(&files, &collection, &principal).unwrap();
    let grants = Grants::default();
    let controls = Controls::new(
        &policy,
        &grants,
        "reader",
        "workspace/default/collection/garden",
    );
    let source = policy.policy().sources.first().unwrap();
    assert!(
        controls
            .caller(
                source,
                &request(
                    "notes",
                    "https://garden.example/docs/start",
                    "2026-10-02T00:00:00Z"
                )
            )
            .is_err()
    );
    fixture.finish();
}
