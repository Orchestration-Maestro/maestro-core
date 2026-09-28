//! Legacy claim records survive the vocabulary migration unchanged.

use super::support::{Scratch, label, legacy, retries, set_of};
use crate::{
    facts::ReviewState,
    scope::ScopeSet,
    store::{Database, Error, pending_migrations},
};
use rusqlite::{Connection, types::Value};
use std::{collections::BTreeSet, ffi::OsString, fs};

/// The legacy columns, including row identity, review history and time.
const CLAIMS: &str = "SELECT rowid, id, collection_id, subject_kind, subject_name, predicate,
    object_type, object_lexeme, conditions_json, version_known, version_start, version_end,
    world_known, world_start, world_end, extractor, profile_digest, review_state,
    support_count, recorded_at FROM claims ORDER BY rowid";

/// The exact stored values of all legacy authority tables and migration history.
fn snapshot(connection: &Connection) -> Vec<Vec<Vec<Value>>> {
    [
        CLAIMS,
        "SELECT * FROM claim_supports
         ORDER BY claim_id, revision_id, block_id, span_start, span_end",
        "SELECT rowid, * FROM claim_sets ORDER BY rowid",
        "SELECT * FROM claim_set_members ORDER BY claim_set_id, ordinal",
        "SELECT * FROM migrations WHERE name < '0013' ORDER BY name",
    ]
    .map(|sql| {
        let mut statement = connection.prepare(sql).unwrap();
        let columns = statement.column_count();
        statement
            .query_map([], |row| (0..columns).map(|index| row.get(index)).collect())
            .unwrap()
            .map(Result::unwrap)
            .collect()
    })
    .to_vec()
}

#[test]
fn valid_legacy_claim_and_set_keep_their_ids_and_supports() {
    let scratch = Scratch::new();
    let (set_id, ids) = legacy(&scratch, None);
    let before = snapshot(&scratch.outside());
    let scopes = ScopeSet::default_workspace();
    let upgraded = Database::open_in(&scratch.0).unwrap();
    assert_eq!(snapshot(&scratch.outside()), before);
    let after = upgraded.claim_set(&scopes, &set_id).unwrap().unwrap();
    assert_eq!(after.id, set_id);
    for ((record, claim), id) in after.claims.iter().zip([label(), retries()]).zip(ids) {
        assert_eq!(record.id, id);
        assert_eq!(record.claim, claim);
        assert_eq!(record.review, ReviewState::Accepted);
        assert_eq!(record.recorded_at, "2026-01-02T03:04:05.678Z");
    }
    assert_eq!(
        upgraded
            .record_claim_set(&scopes, &set_of(vec![label(), retries()]))
            .unwrap(),
        after
    );
    drop(upgraded);
    let reopened = Database::open_in(&scratch.0).unwrap();
    assert_eq!(reopened.claim_set(&scopes, &set_id).unwrap(), Some(after));
    assert!(pending_migrations(&scratch.0).unwrap().is_empty());
    let violations: i64 = scratch
        .outside()
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);
}

#[test]
fn invalid_legacy_kind_refuses_the_whole_upgrade_with_only_ids_and_no_changes() {
    for mode in ["WAL", "DELETE"] {
        assert_refused_without_changes(mode);
    }
}

/// A refused upgrade preserves restored DELETE databases as well as live WAL ones.
fn assert_refused_without_changes(mode: &str) {
    let scratch = Scratch::new();
    let (_, ids) = legacy(&scratch, Some("private-unlisted-kind"));
    scratch
        .outside()
        .pragma_update(None, "journal_mode", mode)
        .unwrap();
    let before = snapshot(&scratch.outside());
    let files = directory_files(&scratch);
    assert!(!files.contains(&OsString::from("kernel.sqlite3-wal")));
    assert!(!files.contains(&OsString::from("kernel.sqlite3-shm")));
    let bytes = fs::read(scratch.0.join("kernel.sqlite3")).unwrap();
    let error = Database::open_in(&scratch.0).unwrap_err();
    assert!(
        matches!(&error, Error::RefusedMigration { name, ids: refused }
        if name == "0013_graph_claim_vocabulary" && refused == &[ids[1].as_str()])
    );
    assert_eq!(
        error.to_string(),
        format!(
            "the kernel database holds records migration 0013_graph_claim_vocabulary cannot \
             carry forward, so it changed nothing: {}",
            ids[1].as_str()
        )
    );
    if mode == "DELETE" {
        assert_eq!(directory_files(&scratch), files);
    }
    assert_eq!(snapshot(&scratch.outside()), before);
    assert_eq!(fs::read(scratch.0.join("kernel.sqlite3")).unwrap(), bytes);
    assert_eq!(
        pending_migrations(&scratch.0).unwrap(),
        [
            "0013_graph_claim_vocabulary",
            "0014_graph_builds",
            "0015_graph_resolution"
        ]
    );
}

/// Names of all entries beside the database, including SQLite sidecars.
fn directory_files(scratch: &Scratch) -> BTreeSet<OsString> {
    fs::read_dir(&scratch.0)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect()
}
