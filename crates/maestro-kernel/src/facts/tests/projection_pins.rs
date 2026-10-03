//! Durable pin decoding, resolution authority and append-only migration proofs.
use super::{
    projection::{attached, projection_lease},
    support::{execute, timing},
};
use crate::facts::{ClaimSet, Error};
use crate::{
    artifact::Digest,
    facts::ResolutionSnapshot,
    facts::{EXACT_RESOLVER_VERSION, InputMismatchKind, ResolutionInput},
    scope::ScopeSet,
    store::Database,
};
use rusqlite::params;

#[test]
fn receipt_missing_unknown_or_malformed_pin_refuses_with_rebuild() {
    for (column, value) in [
        ("resolution_id", None),
        ("resolver_version", None),
        ("settings_identity", None),
        ("frozen_lock", None),
        ("resolution_id", Some("xyzzy")),
        ("resolver_version", Some("unknown/1")),
        ("settings_identity", Some("")),
        ("frozen_lock", Some("xyzzy")),
    ] {
        let (scratch, database, scopes, receipt) = attached();
        let lease = projection_lease(&database, receipt.identity.generation_id);
        database
            .record_projection_ready(&scopes, &receipt, &lease, timing(5).now)
            .unwrap();
        let outside = scratch.outside();
        outside
            .execute_batch(
                "DROP TRIGGER graph_projection_receipts_never_changed;
            PRAGMA ignore_check_constraints=ON; PRAGMA foreign_keys=OFF;",
            )
            .unwrap();
        outside
            .execute(
                &format!("UPDATE graph_projection_receipts SET {column} = ?1"),
                [value],
            )
            .unwrap();
        let error = database
            .projection_ready(&scopes, receipt.identity.generation_id)
            .expect_err("missing, unknown or malformed pin must refuse");
        assert!(
            error
                .to_string()
                .contains("maestro knowledge graph rebuild"),
            "{column}: {error}"
        );
    }
}

#[test]
fn readiness_resolution_must_exist_cover_set_and_match_resolver() {
    let (_scratch, database, scopes, mut receipt) = attached();
    let lease = projection_lease(&database, receipt.identity.generation_id);
    let pin = receipt.resolution_id.clone();
    receipt.resolution_id = Digest::of(b"missing resolution");
    assert!(
        database
            .record_projection_ready(&scopes, &receipt, &lease, timing(5).now)
            .is_err(),
        "missing resolution must refuse"
    );
    receipt.resolution_id = pin;
    receipt.resolver_version = "unknown/1".into();
    assert!(
        database
            .record_projection_ready(&scopes, &receipt, &lease, timing(5).now)
            .is_err(),
        "unknown resolver must refuse"
    );
    receipt.resolver_version = EXACT_RESOLVER_VERSION.into();
    database
        .validate_projection_inputs(
            &scopes,
            &receipt.identity.claim_set_id,
            &receipt.resolution_id,
            &receipt.resolver_version,
        )
        .unwrap();
    assert!(
        database
            .validate_projection_inputs(
                &scopes,
                &Digest::of(b"other set"),
                &receipt.resolution_id,
                &receipt.resolver_version
            )
            .is_err(),
        "resolution must cover the selected set"
    );
    let mut foreign_claim = super::support::label();
    foreign_claim
        .conditions
        .insert("different".into(), "snapshot".into());
    let foreign_set = database
        .record_claim_set(
            &scopes,
            &ClaimSet {
                collection_id: receipt.identity.collection_id.clone(),
                claims: vec![foreign_claim],
            },
        )
        .unwrap();
    let foreign = snapshot(&database, &scopes, &foreign_set.id, EXACT_RESOLVER_VERSION);
    let original = receipt.resolution_id.clone();
    receipt.resolution_id = foreign.id;
    assert!(
        database
            .record_projection_ready(&scopes, &receipt, &lease, timing(5).now)
            .is_err(),
        "snapshot must cover receipt claim set even when resolver matches"
    );
    receipt.resolution_id = original;
    let other = snapshot(
        &database,
        &scopes,
        &receipt.identity.claim_set_id,
        "other/1",
    );
    receipt.resolution_id = other.id;
    assert!(
        database
            .validate_projection_inputs(
                &scopes,
                &receipt.identity.claim_set_id,
                &receipt.resolution_id,
                "other/1"
            )
            .is_err(),
        "unsupported but stored resolver must refuse at build admission"
    );
    assert!(
        database
            .record_projection_ready(&scopes, &receipt, &lease, timing(5).now)
            .is_err(),
        "snapshot resolver must match receipt"
    );
}

#[test]
fn legacy_projection_migration_preserves_rows_triggers_and_refuses_silent_upgrade() {
    let (scratch, database, scopes, receipt) = attached();
    let outside = scratch.outside();
    outside
        .execute_batch(
            "DROP TABLE graph_projection_receipts;
        DELETE FROM migrations WHERE name = '0030_graph_input_pins';",
        )
        .unwrap();
    outside
        .execute_batch(include_str!(
            "../../../migrations/0019_graph_projection.sql"
        ))
        .unwrap();
    outside
        .execute(
            "INSERT INTO graph_projection_receipts
        (generation_id, collection_id, claim_set_id, file_name, schema_version,
        knowledge_edge_count, catalog_dependency_edge_count, entity_fact_count, content_digest)
        VALUES (?1, ?2, ?3, ?4, 'maestro-typed-edges/1', 0, 0, 1, ?5)",
            params![
                receipt.identity.generation_id,
                receipt.identity.collection_id,
                receipt.identity.claim_set_id.as_str(),
                receipt.identity.file_name,
                receipt.identity.content_digest.as_str()
            ],
        )
        .unwrap();
    drop(outside);
    drop(database);
    let migrated = Database::open_in(&scratch.0).unwrap();
    let outside = scratch.outside();
    let stored: (String, Option<String>, String) = outside
        .query_row(
            "SELECT schema_version, resolution_id, content_digest FROM graph_projection_receipts",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        stored,
        (
            "maestro-typed-edges/1".into(),
            None,
            receipt.identity.content_digest.as_str().into()
        ),
        "legacy bytes must not be backfilled"
    );
    assert!(
        migrated
            .projection_ready(&scopes, receipt.identity.generation_id)
            .unwrap_err()
            .to_string()
            .contains("maestro knowledge graph rebuild")
    );
    assert!(
        outside
            .execute(
                "UPDATE graph_projection_receipts SET file_name = 'replace'",
                []
            )
            .is_err()
    );
    assert!(
        outside
            .execute("DELETE FROM graph_projection_receipts", [])
            .is_err()
    );
    assert_eq!(
        outside
            .query_row(
                "SELECT count(*) FROM sqlite_schema
        WHERE type = 'trigger' AND name LIKE 'graph_projection_receipts_%'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        4
    );
    assert_eq!(
        outside
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );
    assert_eq!(
        outside
            .query_row(
                "SELECT count(*) FROM pragma_index_list('graph_projection_receipts')
        WHERE \"unique\" = 1",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}

/// A source-backed snapshot with the algorithm explicitly selected by this proof.
fn snapshot(
    database: &Database,
    scopes: &ScopeSet,
    set: &Digest,
    resolver: &str,
) -> ResolutionSnapshot {
    database
        .record_resolution(
            scopes,
            "projection",
            &ResolutionInput {
                resolver_version: resolver.into(),
                sets: vec![set.clone()],
                previous: None,
                decisions: vec![],
            },
            &|_| Ok(()),
        )
        .unwrap()
}

#[test]
fn old_projection_receipt_refuses_with_rebuild_repair() {
    let (_scratch, database, all, receipt) = attached();
    let lease = projection_lease(&database, receipt.identity.generation_id);
    database
        .record_projection_ready(&all, &receipt, &lease, timing(5).now)
        .unwrap();
    execute(
        &database,
        "DROP TRIGGER graph_projection_receipts_never_changed",
    )
    .unwrap();
    execute(
        &database,
        "UPDATE graph_projection_receipts SET schema_version = 'maestro-typed-edges/1',
        resolution_id = NULL, resolver_version = NULL,
        settings_identity = NULL, frozen_lock = NULL",
    )
    .unwrap();
    let error = database
        .projection_ready(&all, receipt.identity.generation_id)
        .expect_err("old receipt must refuse, not silently upgrade");
    assert!(
        matches!(
            &error,
            Error::ProjectionInputMismatch(InputMismatchKind::Format)
        ),
        "legacy unpinned receipt has typed format mismatch"
    );
    assert!(
        error
            .to_string()
            .contains("maestro knowledge graph rebuild")
    );
}
#[test]
fn raw_receipt_insert_must_match_the_generation_attachment() {
    let (scratch, _database, _all, receipt) = attached();
    let outside = scratch.outside();
    assert!(
        outside
            .execute(
                "INSERT INTO graph_projection_receipts
         (generation_id, collection_id, claim_set_id, file_name, schema_version,
          knowledge_edge_count, catalog_dependency_edge_count, entity_fact_count, content_digest,
          resolution_id, resolver_version, settings_identity, frozen_lock)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                rusqlite::params![
                    receipt.identity.generation_id,
                    receipt.identity.collection_id,
                    receipt.identity.claim_set_id.as_str(),
                    receipt.identity.file_name,
                    receipt.identity.schema_version,
                    1_i64,
                    0_i64,
                    1_i64,
                    receipt.identity.content_digest.as_str(),
                    receipt.resolution_id.as_str(),
                    receipt.resolver_version,
                    receipt.settings_identity.as_str(),
                    receipt.frozen_lock.as_str(),
                ],
            )
            .is_err()
    );
}

#[test]
fn legacy_receipt_corruption_keeps_conflict_category() {
    for assignment in [
        "content_digest =
         'zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz'",
        "claim_set_id = 'bad'",
        "knowledge_edge_count = -1",
        "catalog_dependency_edge_count = -1",
        "entity_fact_count = -1",
        "settings_identity = 'bad'",
        "resolution_id = 'bad'",
        "resolver_version = 'unknown/1'",
        "frozen_lock = 'bad'",
    ] {
        let (scratch, database, scopes, receipt) = attached();
        let lease = projection_lease(&database, receipt.identity.generation_id);
        database
            .record_projection_ready(&scopes, &receipt, &lease, timing(5).now)
            .unwrap();
        let outside = scratch.outside();
        outside
            .execute_batch(
                "DROP TRIGGER graph_projection_receipts_never_changed;
            PRAGMA ignore_check_constraints=ON; PRAGMA foreign_keys=OFF;
            UPDATE graph_projection_receipts SET schema_version = 'maestro-typed-edges/1',
            resolution_id = NULL, resolver_version = NULL,
            settings_identity = NULL, frozen_lock = NULL;",
            )
            .unwrap();
        assert!(matches!(
            database.projection_ready(&scopes, receipt.identity.generation_id),
            Err(Error::ProjectionInputMismatch(InputMismatchKind::Format))
        ));
        let legacy = database
            .projection_receipt_identity(&scopes, receipt.identity.generation_id)
            .unwrap()
            .unwrap();
        let mut expected = receipt.identity.clone();
        expected.schema_version = "maestro-typed-edges/1".into();
        assert_eq!(legacy, expected);
        outside
            .execute_batch(&format!(
                "UPDATE graph_projection_receipts SET {assignment}"
            ))
            .unwrap();
        assert!(
            matches!(
                database.projection_ready(&scopes, receipt.identity.generation_id),
                Err(Error::Conflict(_))
            ),
            "{assignment}: corruption must not become format mismatch"
        );
        assert!(
            matches!(
                database.projection_receipt_identity(&scopes, receipt.identity.generation_id),
                Err(Error::Conflict(_))
            ),
            "{assignment}: cleanup identity must also reject corruption"
        );
    }
}
