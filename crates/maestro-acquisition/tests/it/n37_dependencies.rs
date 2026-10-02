//! Corrupt, pending and incompatible dependencies cannot be reused as completed.
use super::{
    n12_parents::derive,
    n12_support::Fixture,
    n37_support::{item, scope},
};
use maestro_kernel::acquisition::{Captures, Frontier, LeaseRequest, Representation};
use rusqlite::Connection;
use std::time::Duration;

#[test]
fn n37_unacknowledged_prerequisite_is_not_a_completed_stage() {
    let mut fixture = Fixture::new();
    let parent = fixture.prepare().unwrap();
    derive(&mut fixture, parent, Representation::SelectedHtml);
    assert!(
        fixture.prepare().is_err(),
        "prepared prerequisite promoted before acknowledgement"
    );
}

#[test]
fn n37_dependency_verification_checks_current_scope_and_source() {
    for changed in ["scope", "source"] {
        let mut fixture = Fixture::new();
        let parent_item = fixture.context.item.item;
        let parent = fixture.prepare().unwrap();
        fixture
            .db
            .acknowledge_capture(&fixture.context, parent)
            .unwrap();
        derive(&mut fixture, parent, Representation::SelectedHtml);
        let child = fixture.prepare().unwrap();
        fixture
            .db
            .acknowledge_capture(&fixture.context, child)
            .unwrap();
        let sql = Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
        if changed == "scope" {
            sql.execute(
                "UPDATE acquisition_evidence
                 SET scope = 'workspace/default/collection/other' WHERE id = ?1",
                [parent.to_string()],
            )
            .unwrap();
        } else {
            fixture
                .db
                .lease_source(
                    "other",
                    &scope(),
                    LeaseRequest {
                        holder: "other",
                        now: fixture.context.now,
                        term: Duration::from_secs(30),
                    },
                )
                .unwrap();
            sql.execute(
                "UPDATE acquisition_frontier SET source = 'other' WHERE id = ?1",
                [parent_item.to_string()],
            )
            .unwrap();
        }
        assert_eq!(
            fixture.db.capture_for(&scope(), &item(&fixture)).unwrap(),
            None,
            "changed parent {changed} reused"
        );
        assert!(fixture.db.read_capture(&fixture.context, child, 4).is_err());
    }
}

#[test]
fn n37_dependency_representation_matrix_remains_closed() {
    let mut fixture = Fixture::new();
    let parent = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, parent)
        .unwrap();
    derive(&mut fixture, parent, Representation::ApiRecord);
    let api = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, api)
        .unwrap();
    derive(&mut fixture, api, Representation::SelectedHtml);
    assert!(
        fixture.prepare().is_err(),
        "undeclared API-to-HTML derivation reused"
    );
}

#[test]
fn n37_capture_dependency_cycle_refuses_without_recursive_stack_growth() {
    let mut fixture = Fixture::new();
    let parent = fixture.prepare().unwrap();
    // Model corrupted kernel metadata while retaining intact artifact hashes.
    fixture.envelope.parent = Some(parent);
    fixture.envelope.representation = Representation::SelectedHtml;
    let digest = fixture
        .db
        .put(
            &serde_json::to_vec(&fixture.envelope).unwrap(),
            "application/json",
        )
        .unwrap();
    let sql = Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    sql.execute(
        "UPDATE acquisition_evidence SET artifact = ?2 WHERE id = ?1",
        [parent.to_string(), digest.as_str().to_owned()],
    )
    .unwrap();
    sql.execute(
        "UPDATE acquisition_frontier SET capture = ?2 WHERE id = ?1",
        [
            fixture.context.item.item.to_string(),
            digest.as_str().to_owned(),
        ],
    )
    .unwrap();
    let row = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        1,
    )
    .unwrap()
    .remove(0);
    assert!(fixture.db.verify_capture(&scope(), &row, parent).is_err());
}
