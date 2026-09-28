//! What the schema refuses whoever writes: replacing, changing or deleting
//! a recorded claim, support, set or member, but a claim's review state;
//! a support or a member after the write; and a support or a member from
//! another collection.

use super::support::{Scratch, counts, execute, label, retries, set_of};
use crate::{scope::ScopeSet, store::Database};
use rusqlite::Connection;

/// Asserts that `statement`, run on the kernel's writer, is refused with
/// `message`.
fn refused(database: &Database, statement: &str, message: &str) {
    let error = execute(database, statement).unwrap_err();
    assert!(
        format!("{error:?}").contains(message),
        "{statement}: {error:?}"
    );
}

/// Asserts that `statement`, run on `connection`, is refused with `message`.
fn refused_outside(connection: &Connection, statement: &str, message: &str) {
    let error = connection.execute(statement, []).unwrap_err();
    assert!(
        format!("{error:?}").contains(message),
        "{statement}: {error:?}"
    );
}

/// The database of `scratch` with two recorded sets: `label` alone, then
/// `retries` alone.
fn two_sets(scratch: &Scratch) -> Database {
    let database = scratch.open();
    for claim in [label(), retries()] {
        database
            .record_claim_set(&ScopeSet::default_workspace(), &set_of(vec![claim]))
            .unwrap();
    }
    database
}

#[test]
fn a_recorded_claim_set_is_never_replaced_changed_or_deleted() {
    let scratch = Scratch::new();
    let database = two_sets(&scratch);
    let label_set = "(SELECT claim_set_id FROM claim_set_members
                      JOIN claims ON claims.id = claim_id WHERE subject_name = 'label')";
    let retries_claim = "(SELECT id FROM claims WHERE subject_name = 'retries')";
    let cases = [
        (
            "INSERT OR REPLACE INTO claims SELECT * FROM claims".to_owned(),
            "a claim is immutable: it is never replaced",
        ),
        (
            "UPDATE claims SET object_lexeme = 'tea'".to_owned(),
            "a claim is immutable: only its review state moves",
        ),
        (
            "DELETE FROM claims".to_owned(),
            "a claim is immutable: it is never deleted",
        ),
        (
            "INSERT OR REPLACE INTO claim_supports SELECT * FROM claim_supports".to_owned(),
            "a claim's supports are frozen",
        ),
        (
            "INSERT INTO claim_supports SELECT claim_id, revision_id, 'block-more', span_start,
               span_end, quote_digest FROM claim_supports"
                .to_owned(),
            "a claim's supports are frozen",
        ),
        (
            "UPDATE claim_supports SET span_end = span_end - 1".to_owned(),
            "a claim support never changes",
        ),
        (
            "DELETE FROM claim_supports".to_owned(),
            "a claim support is never deleted",
        ),
        (
            "INSERT OR REPLACE INTO claim_sets SELECT * FROM claim_sets".to_owned(),
            "a claim set is never replaced",
        ),
        (
            "UPDATE claim_sets SET member_count = 2".to_owned(),
            "a claim set never changes",
        ),
        (
            "DELETE FROM claim_sets".to_owned(),
            "a claim set is never deleted",
        ),
        (
            "INSERT OR REPLACE INTO claim_set_members SELECT * FROM claim_set_members".to_owned(),
            "a claim set's members are frozen",
        ),
        // Another recorded claim of the collection, at an ordinal below the
        // count: only the frozen membership refuses it before its key does.
        (
            format!("INSERT INTO claim_set_members VALUES ({label_set}, 0, {retries_claim})"),
            "a claim set's members are frozen",
        ),
        (
            "UPDATE claim_set_members SET ordinal = 1".to_owned(),
            "a claim set member never changes",
        ),
        (
            "DELETE FROM claim_set_members".to_owned(),
            "a claim set member is never deleted",
        ),
    ];
    for (statement, message) in &cases {
        refused(&database, statement, message);
    }
    assert_eq!(counts(&scratch), [2, 2, 2, 2]);
}

#[test]
fn every_claim_column_but_its_review_state_is_frozen() {
    let scratch = Scratch::new();
    let database = two_sets(&scratch);
    let columns: Vec<String> = {
        let reader = scratch.outside();
        let mut statement = reader
            .prepare("SELECT name FROM pragma_table_info('claims') ORDER BY cid")
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(columns.len(), 19, "{columns:?}");
    for column in columns.iter().filter(|column| *column != "review_state") {
        refused(
            &database,
            &format!("UPDATE claims SET {column} = {column}"),
            "a claim is immutable: only its review state moves",
        );
    }
    assert_eq!(
        execute(&database, "UPDATE claims SET review_state = 'flagged'").unwrap(),
        2
    );
}

#[test]
fn a_replace_is_refused_on_a_connection_without_recursive_triggers() {
    let scratch = Scratch::new();
    drop(two_sets(&scratch));
    let outside = scratch.outside();
    let recursive: bool = outside
        .pragma_query_value(None, "recursive_triggers", |row| row.get(0))
        .unwrap();
    assert!(!recursive, "the delete triggers must not fire on a replace");
    let label_set = "(SELECT claim_set_id FROM claim_set_members
                      JOIN claims ON claims.id = claim_id WHERE subject_name = 'label')";
    let cases = [
        (
            "INSERT OR REPLACE INTO claims SELECT id, collection_id, subject_kind,
               subject_name, predicate, object_type, 'tea', conditions_json, version_known,
               version_start, version_end, world_known, world_start, world_end, extractor,
               profile_digest, review_state, support_count, recorded_at
             FROM claims WHERE subject_name = 'label'"
                .to_owned(),
            "a claim is immutable: it is never replaced",
        ),
        (
            "INSERT OR REPLACE INTO claim_sets SELECT id, collection_id, member_count + 1
             FROM claim_sets"
                .to_owned(),
            "a claim set is never replaced",
        ),
        (
            "INSERT OR REPLACE INTO claim_supports SELECT claim_id, revision_id, block_id,
               span_start, span_end, 'changed' FROM claim_supports"
                .to_owned(),
            "a claim's supports are frozen",
        ),
        (
            format!(
                "INSERT OR REPLACE INTO claim_set_members
                 SELECT {label_set}, 0, id FROM claims WHERE subject_name = 'retries'"
            ),
            "a claim set's members are frozen",
        ),
    ];
    let snapshot = |connection: &Connection| -> String {
        connection
            .query_row(
                "SELECT group_concat(object_lexeme) || '|' ||
                   (SELECT group_concat(member_count) FROM claim_sets) || '|' ||
                   (SELECT group_concat(quote_digest) FROM claim_supports) || '|' ||
                   (SELECT group_concat(claim_set_id || claim_id) FROM claim_set_members)
                 FROM claims",
                [],
                |row| row.get(0),
            )
            .unwrap()
    };
    let before = snapshot(&outside);
    for (statement, message) in &cases {
        refused_outside(&outside, statement, message);
    }
    assert_eq!(snapshot(&outside), before);
}

/// Records, with raw SQL, the claim `id` of `collection` that holds one
/// support.
fn raw_claim(database: &Database, id: &str, collection: &str) {
    execute(
        database,
        &format!(
            "INSERT INTO claims (id, collection_id, subject_kind, subject_name, predicate,
               object_type, object_lexeme, conditions_json, version_known, world_known,
               extractor, profile_digest, support_count)
             VALUES ('{id}', '{collection}', 'Parameter', 'raw', 'DEFAULTS_TO', 'text', 'x',
               '{{}}', 0, 0, 'raw/1', 'p', 1)"
        ),
    )
    .unwrap();
}

#[test]
fn a_support_quotes_only_a_revision_of_its_claims_collection() {
    let scratch = Scratch::new();
    let database = scratch.open();
    raw_claim(&database, "raw-graph", "graph");
    refused(
        &database,
        "INSERT INTO claim_supports VALUES ('raw-graph', 'rev-o', 'b', 0, 5, 'q')",
        "a claim is supported only by revisions of its collection",
    );
    execute(
        &database,
        "INSERT INTO claim_supports VALUES ('raw-graph', 'rev-a', 'b', 0, 5, 'q')",
    )
    .unwrap();
}

#[test]
fn a_set_holds_only_claims_of_its_collection_below_its_count() {
    let scratch = Scratch::new();
    let database = scratch.open();
    raw_claim(&database, "raw-graph", "graph");
    raw_claim(&database, "raw-other", "other");
    execute(
        &database,
        "INSERT INTO claim_sets VALUES ('raw-set', 'graph', 2)",
    )
    .unwrap();
    let message = "a claim set holds claims of its collection, at ordinals below its count";
    refused(
        &database,
        "INSERT INTO claim_set_members VALUES ('raw-set', 0, 'raw-other')",
        message,
    );
    refused(
        &database,
        "INSERT INTO claim_set_members VALUES ('raw-set', 7, 'raw-graph')",
        message,
    );
    execute(
        &database,
        "INSERT INTO claim_set_members VALUES ('raw-set', 1, 'raw-graph')",
    )
    .unwrap();
}
