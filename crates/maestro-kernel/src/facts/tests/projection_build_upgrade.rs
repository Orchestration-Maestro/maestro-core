//! 0031 imports retained authority without creating pins, jobs or search events.
use super::projection_schema_support::Fixture;
use rusqlite::types::Value;

/// Preserve complete rows, not a selected subset of the search lifecycle.
fn rows(fixture: &Fixture, table: &str) -> Vec<Vec<Value>> {
    let mut statement = fixture
        .connection
        .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
        .unwrap();
    let columns = statement.column_count();
    statement
        .query_map([], |row| {
            (0..columns).map(|column| row.get(column)).collect()
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn projection_build_upgrade_preserves_legacy_and_pinned_authority_in_every_retained_state() {
    for legacy in [false, true] {
        for state in ["verified", "published", "retired", "failed"] {
            let fixture = Fixture::new(true);
            fixture
                .connection
                .execute_batch(
                    "DROP TRIGGER graph_projection_receipts_never_changed;
                UPDATE graph_projection_receipts SET verified_at = '2000-01-02T03:04:05.678Z';",
                )
                .unwrap();
            if legacy {
                fixture
                    .connection
                    .execute_batch(
                        "UPDATE graph_projection_receipts
                    SET schema_version = 'maestro-typed-edges/1', resolution_id = NULL,
                    resolver_version = NULL, settings_identity = NULL, frozen_lock = NULL;",
                    )
                    .unwrap();
            }
            // Restore the exact trigger expected from 0030 after the synthetic legacy setup.
            fixture
                .connection
                .execute_batch(
                    "CREATE TRIGGER graph_projection_receipts_never_changed
                BEFORE UPDATE ON graph_projection_receipts
                BEGIN SELECT RAISE(ABORT, 'immutable'); END;",
                )
                .unwrap();
            fixture.set_state(state);
            let old = rows(&fixture, "graph_projection_receipts");
            let untouched = [
                "generations",
                "graph_attachments",
                "jobs",
                "events",
                "generation_search",
            ]
            .map(|table| (table, rows(&fixture, table)));
            fixture.migrate();
            for (table, before) in untouched {
                assert_eq!(rows(&fixture, table), before, "{table}");
            }
            let mut statement = fixture
                .connection
                .prepare(
                    "SELECT b.generation_id, b.collection_id, b.claim_set_id, r.file_name,
                    b.schema_version, r.knowledge_edge_count, r.catalog_dependency_edge_count,
                    r.entity_fact_count, r.content_digest, b.resolution_id, b.resolver_version,
                    b.settings_identity, b.frozen_lock, r.verified_at
                 FROM graph_projection_builds b JOIN graph_projection_receipts r USING (build_id)",
                )
                .unwrap();
            let imported: Vec<Vec<Value>> = statement
                .query_map([], |row| (0..14).map(|column| row.get(column)).collect())
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            assert_eq!(imported, old, "legacy={legacy}, state={state}");
            assert_eq!(fixture.head(), fixture.generation());
            let historical: bool = fixture
                .connection
                .query_row(
                    "SELECT project_job_id IS NULL
                AND expected_active_build_id IS NULL FROM graph_projection_builds",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(historical);
            fixture.foreign_keys_clean();
        }
    }
}

#[test]
fn projection_build_upgrade_never_fabricates_pins_for_a_legacy_replacement() {
    let fixture = Fixture::new(true);
    fixture
        .connection
        .execute_batch(
            "DROP TRIGGER graph_projection_receipts_never_changed;
        UPDATE graph_projection_receipts SET schema_version = 'maestro-typed-edges/1',
            resolution_id = NULL, resolver_version = NULL,
                    settings_identity = NULL, frozen_lock = NULL;
        CREATE TRIGGER graph_projection_receipts_never_changed
        BEFORE UPDATE ON graph_projection_receipts
            BEGIN SELECT RAISE(ABORT, 'immutable'); END;",
        )
        .unwrap();
    fixture.migrate();
    let build = fixture.reserve(Some(fixture.generation()));
    fixture.insert_receipt(build, "replacement.db").unwrap();
    fixture.advance(build).unwrap();
    let pins_absent: bool = fixture
        .connection
        .query_row(
            "SELECT resolution_id IS NULL
        AND resolver_version IS NULL AND settings_identity IS NULL AND frozen_lock IS NULL
        FROM graph_projection_builds WHERE build_id = ?1",
            [fixture.generation()],
            |row| row.get(0),
        )
        .unwrap();
    assert!(pins_absent);
    fixture.foreign_keys_clean();
}

#[test]
fn projection_build_upgrade_malformed_pins_roll_back_the_entire_migration() {
    let fixture = Fixture::new(true);
    fixture
        .connection
        .execute_batch(
            "DROP TRIGGER graph_projection_receipts_never_changed;
        PRAGMA ignore_check_constraints=ON;
        UPDATE graph_projection_receipts SET settings_identity = 'bad';
        PRAGMA ignore_check_constraints=OFF;
        CREATE TRIGGER graph_projection_receipts_never_changed
        BEFORE UPDATE ON graph_projection_receipts
            BEGIN SELECT RAISE(ABORT, 'immutable'); END;",
        )
        .unwrap();
    let before = rows(&fixture, "graph_projection_receipts");
    fixture.connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert!(
        fixture
            .connection
            .execute_batch(include_str!(
                "../../../migrations/0031_graph_projection_builds.sql"
            ))
            .is_err()
    );
    fixture.connection.execute_batch("ROLLBACK").unwrap();
    assert_eq!(rows(&fixture, "graph_projection_receipts"), before);
    let new_tables: i64 = fixture
        .connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema
        WHERE name IN ('graph_projection_builds', 'graph_projection_active',
            'graph_projection_receipts_0030')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(new_tables, 0);
}

#[test]
fn projection_build_upgrade_retains_nonhex_digest_for_strict_runtime_corruption_decoding() {
    let fixture = Fixture::new(true);
    let digest = "z".repeat(64);
    fixture
        .connection
        .execute_batch("DROP TRIGGER graph_projection_receipts_never_changed")
        .unwrap();
    fixture
        .connection
        .execute(
            "UPDATE graph_projection_receipts SET content_digest = ?1",
            [&digest],
        )
        .unwrap();
    fixture
        .connection
        .execute_batch(
            "CREATE TRIGGER graph_projection_receipts_never_changed
        BEFORE UPDATE ON graph_projection_receipts
        BEGIN SELECT RAISE(ABORT, 'immutable'); END;",
        )
        .unwrap();
    fixture.migrate();
    let retained: String = fixture
        .connection
        .query_row(
            "SELECT content_digest FROM graph_projection_receipts",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(retained, digest);
}
