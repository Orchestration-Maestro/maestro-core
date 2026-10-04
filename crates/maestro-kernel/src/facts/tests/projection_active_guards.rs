//! Active heads select complete receipts, never reserved or historical storage.
use super::projection_schema_support::Fixture;
use rusqlite::params;

#[test]
fn projection_active_guards_initial_head_requires_a_receipted_verified_build() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let build = fixture.reserve(None);
    let insert = format!(
        "INSERT INTO graph_projection_active VALUES ({}, {build})",
        fixture.generation()
    );
    assert!(
        fixture.connection.execute_batch(&insert).is_err(),
        "receiptless pointer"
    );
    fixture.insert_receipt(build, "initial.db").unwrap();
    fixture.set_state("published");
    assert!(
        fixture.connection.execute_batch(&insert).is_err(),
        "published without an initial head"
    );
    fixture.set_state("verified");
    fixture.connection.execute_batch(&insert).unwrap();
    assert!(
        fixture
            .connection
            .execute_batch(&insert.replace("INSERT INTO", "INSERT OR REPLACE INTO"))
            .is_err(),
        "replacement INSERT with recursive triggers off"
    );
    assert!(fixture.advance(build).is_err(), "same-head UPDATE");
    assert!(
        fixture
            .connection
            .execute_batch("DELETE FROM graph_projection_active")
            .is_err()
    );
    assert_eq!(fixture.head(), build);
    fixture.foreign_keys_clean();
}

#[test]
fn projection_active_guards_updates_require_receipt_exact_predecessor_and_eligible_state() {
    let fixture = Fixture::new(true);
    fixture.migrate();
    let old = fixture.generation();
    let first = fixture.reserve(Some(old));
    let stale = fixture.reserve(Some(old));
    assert!(fixture.advance(first).is_err(), "receiptless advance");
    fixture.insert_receipt(first, "first.db").unwrap();
    fixture.insert_receipt(stale, "stale.db").unwrap();
    for state in ["retired", "failed", "building"] {
        fixture.set_state(state);
        assert!(fixture.advance(first).is_err(), "{state}");
        assert_eq!(fixture.head(), old);
    }
    fixture.set_state("published");
    fixture.advance(first).unwrap();
    assert!(
        fixture.advance(stale).is_err(),
        "wrong expected predecessor"
    );
    assert!(
        fixture.advance(old).is_err(),
        "rollback to retained receipt"
    );
    assert!(
        fixture
            .connection
            .execute(
                "UPDATE graph_projection_active SET generation_id = ?1",
                [99999]
            )
            .is_err()
    );
    assert_eq!(fixture.head(), first);
    fixture.foreign_keys_clean();
}

#[test]
fn projection_active_guards_failed_cas_rolls_back_the_receipt_in_the_same_transaction() {
    let fixture = Fixture::new(true);
    fixture.migrate();
    let build = fixture.reserve(Some(fixture.generation()));
    fixture.connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    fixture.insert_receipt(build, "uncommitted.db").unwrap();
    let rows = fixture
        .connection
        .execute(
            "UPDATE graph_projection_active SET build_id = ?1
         WHERE generation_id = ?2 AND build_id = ?3",
            params![build, fixture.generation(), 99999],
        )
        .unwrap();
    assert_eq!(rows, 0);
    fixture.connection.execute_batch("ROLLBACK").unwrap();
    assert_eq!(fixture.count("graph_projection_receipts"), 1);
    assert_eq!(fixture.head(), fixture.generation());
}

#[test]
fn projection_active_guards_cross_generation_pointer_is_refused_even_without_foreign_keys() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let build = fixture.reserve(None);
    fixture.insert_receipt(build, "candidate.db").unwrap();
    fixture
        .connection
        .execute_batch("PRAGMA foreign_keys=OFF")
        .unwrap();
    assert!(
        fixture
            .connection
            .execute(
                "INSERT INTO graph_projection_active VALUES (?1, ?2)",
                params![99999, build]
            )
            .is_err()
    );
    assert_eq!(fixture.count("graph_projection_active"), 0);
}
