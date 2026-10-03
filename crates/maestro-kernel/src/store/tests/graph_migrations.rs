//! Graph migrations upgrade populated legacy stores and roll back failed DDL.

use super::support::Scratch;
use crate::store::migration::{MIGRATIONS, apply};
use rusqlite::Connection;

/// Legacy records preserved across the graph migration.
const LEGACY: &str = "
INSERT INTO collections VALUES ('c','Synthetic','private','{}');
INSERT INTO sources VALUES ('c','s','import',NULL,'synthetic','{}');
INSERT INTO documents VALUES ('d','c','s','page.md');
INSERT INTO revisions(id,document_id,original_digest,canonical_digest,status,metadata_json)
VALUES ('r','d','source','canonical','valid','{}');
INSERT INTO chunk_sets VALUES ('set','c','mapped-structural-chunks/4','counter','building',NULL);
INSERT INTO chunks VALUES ('set','chunk','r',NULL,'input',1,0,1);
INSERT INTO generations(collection_id,chunk_set_id,embedding_profile,sparse_profile)
VALUES ('c','set','dense','sparse');";

/// Logical schema, independent of creation time and root page allocation.
fn schema(connection: &Connection) -> Vec<(String, String)> {
    connection
        .prepare("SELECT name,sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY name")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn graph_migrations_fresh_and_populated_0011_have_identical_schemas() {
    assert!(
        MIGRATIONS
            .iter()
            .any(|(name, _)| name.ends_with("unit_graphs"))
    );
    assert!(
        MIGRATIONS
            .iter()
            .any(|(name, _)| name.ends_with("retrieval_representations"))
    );
    let legacy: Vec<_> = MIGRATIONS
        .iter()
        .copied()
        .filter(|(name, _)| *name < "0012")
        .collect();
    let upgraded = Scratch::new();
    drop(upgraded.open_with(&legacy).unwrap());
    upgraded.outside().execute_batch(LEGACY).unwrap();
    drop(upgraded.open());
    let fresh = Scratch::new();
    drop(fresh.open());
    assert_eq!(schema(&upgraded.outside()), schema(&fresh.outside()));
    let outside = upgraded.outside();
    assert_eq!(
        outside
            .query_row("SELECT count(*) FROM chunks", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(
        !outside
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap()
    );
}

#[test]
fn graph_migrations_failed_ddl_and_ledger_are_rolled_back() {
    let legacy: Vec<_> = MIGRATIONS
        .iter()
        .copied()
        .filter(|(name, _)| *name < "0012")
        .collect();
    let scratch = Scratch::new();
    drop(scratch.open_with(&legacy).unwrap());
    let mut connection = scratch.outside();
    connection.execute_batch(LEGACY).unwrap();
    let before = schema(&connection);
    let (name, sql) = MIGRATIONS
        .iter()
        .find(|(name, _)| name.ends_with("unit_graphs"))
        .unwrap();
    let interrupted = format!("{sql}\nINSERT INTO missing_table VALUES (1);");
    assert!(apply(&mut connection, name, &interrupted).is_err());
    assert_eq!(schema(&connection), before);
    assert!(
        !connection
            .prepare("SELECT 1 FROM migrations WHERE name=?1")
            .unwrap()
            .exists([name])
            .unwrap()
    );
    apply(&mut connection, name, sql).unwrap();
}

#[test]
fn graph_migrations_refuse_sql_update_delete_and_replace() {
    let scratch = Scratch::new();
    drop(scratch.open());
    let connection = scratch.outside();
    connection
        .execute_batch("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=OFF;")
        .unwrap();
    connection.execute_batch(LEGACY).unwrap();
    connection
        .execute_batch(
            "
INSERT INTO artifacts(digest,bytes,media)
VALUES ('graph',1,'json'),('mapping',1,'json'),('source',1,'text'),('shard',1,'json');
INSERT INTO chunk_set_profiles
VALUES ('c','set','mapped-structural-chunks/4','profile',
'canonical-context-parts/v3','preparation','counter','v2_unit');
INSERT INTO revision_unit_graphs
VALUES ('c','set','r','d','graph','mapping',
'maestro-unit-graph/1','source');
INSERT INTO representation_sets
VALUES ('c','set','rep','profile','dense','sparse','dense-sparse/1','building');
INSERT INTO representation_revisions
VALUES ('c','set','rep','r','shard');",
        )
        .unwrap();
    connection.execute_batch(
        "INSERT INTO revisions(id,document_id,original_digest,canonical_digest,status,metadata_json)
         VALUES ('failed','d','source','canonical','failed','{}');",
    ).unwrap();
    assert!(
        connection
            .execute_batch(
                "INSERT INTO revision_unit_graphs
         VALUES ('c','set','failed','d','graph','mapping','maestro-unit-graph/1','source')"
            )
            .is_err()
    );
    connection
        .execute_batch(
            "INSERT INTO chunk_sets
         VALUES ('badset','c','mapped-structural-chunks/4','counter','building',NULL);",
        )
        .unwrap();
    assert!(
        connection
            .execute_batch(
                "INSERT INTO chunk_set_profiles
         VALUES ('c','badset','wrong','profile','canonical-context-parts/v3',
         'preparation','counter','v2_unit')"
            )
            .is_err()
    );
    for table in [
        "chunk_set_profiles",
        "revision_unit_graphs",
        "representation_sets",
        "representation_revisions",
    ] {
        for sql in [
            format!("UPDATE {table} SET collection_id=collection_id"),
            format!("DELETE FROM {table}"),
            format!("INSERT OR REPLACE INTO {table} SELECT * FROM {table}"),
        ] {
            assert!(connection.execute_batch(&sql).is_err(), "{sql}");
        }
    }
    assert!(
        connection
            .execute_batch("INSERT INTO generation_representations VALUES (1,'c','set','rep')")
            .is_err()
    );
    connection
        .execute_batch("UPDATE representation_sets SET state='complete';")
        .unwrap();
    assert!(
        connection
            .execute_batch("UPDATE representation_sets SET state='building'")
            .is_err()
    );
    assert!(
        connection
            .execute_batch("UPDATE representation_sets SET state='failed'")
            .is_err()
    );
    connection
        .execute_batch(
            "UPDATE chunk_sets SET state='complete',manifest_digest='manifest';
            INSERT INTO generation_representations VALUES (1,'c','set','rep');",
        )
        .unwrap();
    for sql in [
        "UPDATE generation_representations SET generation_id=generation_id",
        "DELETE FROM generation_representations",
        "INSERT OR REPLACE INTO generation_representations
         SELECT * FROM generation_representations",
    ] {
        assert!(connection.execute_batch(sql).is_err(), "{sql}");
    }
}

#[test]
fn preflight_checks_vocabulary_only_when_the_upgrade_is_requested() {
    use crate::store::{Error, migration::preflight};
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(concat!(
            "CREATE TABLE migrations(name TEXT);",
            "INSERT INTO migrations VALUES ('0012_graph_claims');",
            "CREATE TABLE claims(id TEXT, subject_kind TEXT);",
            "INSERT INTO claims VALUES ('legacy-id', 'unlisted-kind');",
        ))
        .unwrap();
    let vocabulary = [("0013_graph_claim_vocabulary", "")];
    let result = preflight(&connection, &vocabulary);
    let expected = "0013_graph_claim_vocabulary";
    assert!(matches!(result, Err(Error::RefusedMigration { name, ids })
        if name == expected && ids == ["legacy-id"]));
    assert!(preflight(&connection, &[("0012_graph_claims", "")]).is_ok());
}
