//! Active head selection is shared by runtime readiness, identity and health.
use super::{
    projection::{attached, projection_lease},
    projection_reservation::{begin, lease, request},
    projection_schema_support::Fixture,
    support::timing,
};
use crate::{
    facts::{Error, PROJECTION_REBUILD_REPAIR, projection_records},
    scope::ScopeSet,
};

#[test]
fn projection_active_reads_select_replacement_not_retained_receipt() {
    let fixture = Fixture::new(true);
    fixture.migrate();
    let scopes = ScopeSet::default_workspace();
    let previous = fixture.head();
    let request = request(&fixture, Some(previous));
    let lease = lease(&fixture, &request);
    let build = begin(&fixture, &request, &lease).unwrap();
    let mut output = fixture.receipt.clone();
    output.identity.build_id = build.build_id;
    output.identity.file_name = "replacement.lbdb".into();
    output.identity.schema_version = "maestro-typed-edges/3".into();
    fixture
        .database
        .record_projection_ready(&scopes, &output, &lease, timing(7).now)
        .unwrap();
    assert_eq!(
        fixture
            .database
            .projection_ready(&scopes, fixture.generation())
            .unwrap(),
        Some(output.clone())
    );
    assert_eq!(
        fixture
            .database
            .projection_receipt_identity(&scopes, fixture.generation())
            .unwrap(),
        Some(output.identity)
    );
    assert_eq!(
        fixture
            .database
            .projection_build_receipt(&scopes, fixture.generation(), previous)
            .unwrap(),
        Some(fixture.receipt.clone())
    );
}

#[test]
fn projection_active_reads_missing_head_is_not_dangling_authority() {
    for corruption in ["missing", "build", "receipt", "generation", "collection"] {
        let fixture = Fixture::new(true);
        fixture.migrate();
        let scopes = ScopeSet::default_workspace();
        fixture.set_state("published");
        fixture
            .connection
            .execute_batch(
                "PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;
             DROP TRIGGER graph_projection_active_never_deleted;
             DROP TRIGGER graph_projection_builds_never_deleted;
             DROP TRIGGER graph_projection_receipts_never_deleted;
             DROP TRIGGER graph_projection_builds_never_changed;",
            )
            .unwrap();
        let sql = match corruption {
            "missing" => "DELETE FROM graph_projection_active",
            "build" => "DELETE FROM graph_projection_builds",
            "receipt" => "DELETE FROM graph_projection_receipts",
            "generation" => "UPDATE graph_projection_builds SET generation_id = 999",
            _ => "UPDATE graph_projection_builds SET collection_id = 'other'",
        };
        fixture.connection.execute_batch(sql).unwrap();
        let ready = fixture
            .database
            .projection_ready(&scopes, fixture.generation());
        let identity = fixture
            .database
            .projection_receipt_identity(&scopes, fixture.generation());
        let inventory = projection_records::active(&fixture.connection, &scopes, None);
        if corruption == "missing" {
            assert_eq!(ready.unwrap(), None);
            assert_eq!(identity.unwrap(), None);
            assert!(inventory.unwrap()[0].2.is_none());
        } else {
            assert!(matches!(ready, Err(Error::Conflict(_))), "{corruption}");
            assert!(matches!(identity, Err(Error::Conflict(_))), "{corruption}");
            assert!(matches!(inventory, Err(Error::Conflict(_))), "{corruption}");
        }
    }
}

#[test]
fn projection_readiness_rejects_a_nonpositive_generation() {
    let (_scratch, database, all, mut receipt) = attached();
    let lease = projection_lease(&database, receipt.identity.generation_id);
    receipt.identity.generation_id = 0;
    assert!(matches!(
        database.record_projection_ready(&all, &receipt, &lease, timing(5).now),
        Err(Error::Conflict(_))
    ));
}

#[test]
fn projection_readiness_rejects_a_path_instead_of_a_owned_filename() {
    let (_scratch, database, all, mut receipt) = attached();
    let lease = projection_lease(&database, receipt.identity.generation_id);
    for name in [
        "../outside.db",
        "safe/outside.db",
        ".leading.db",
        "",
        "space name.db",
    ] {
        receipt.identity.file_name = name.to_owned();
        let error = database
            .record_projection_ready(&all, &receipt, &lease, timing(5).now)
            .unwrap_err();
        let Error::Conflict(detail) = error else {
            panic!("{name}: {error}");
        };
        assert_eq!(
            detail,
            format!("invalid projection receipt identity; {PROJECTION_REBUILD_REPAIR}"),
            "{name}"
        );
    }
    assert_eq!(
        database
            .projection_ready(&all, receipt.identity.generation_id)
            .unwrap(),
        None
    );
}
