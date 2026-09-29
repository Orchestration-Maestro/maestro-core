//! Synthetic source, graph and kernel records shared by persistence tests.
use crate::{
    artifact::Digest,
    scope::ScopeSet,
    store::{Database, Error},
    unit_graph::DeliveryGraph,
};
use maestro_test_scratch::scratch_directory;
use rusqlite::params;
use std::{fs, path::PathBuf};

/// Scratch path outlives the database returned by `open`.
pub(super) struct Scratch(PathBuf);
impl Scratch {
    /// Allocates an isolated directory.
    pub(super) fn new() -> Self {
        Self(scratch_directory().unwrap())
    }
    /// Records synthetic legacy rows, real source/input artifacts and one chunk.
    pub(super) fn open(&self) -> Database {
        self.open_source(include_bytes!("../../../tests/fixtures/unit-graph-v1.txt"))
    }
    /// Records the same synthetic identities with caller-selected source bytes.
    pub(super) fn open_source(&self, source: &[u8]) -> Database {
        let db = Database::open_in(&self.0).unwrap();
        let original = db.put(source, "text/plain").unwrap();
        let input = db
            .put(b"Caption\n| A |\n| 1 |\n| 2 |\n", "text/plain")
            .unwrap();
        db.write::<_, Error>(|tx| {
            tx.execute_batch(
                "
INSERT INTO collections VALUES ('collection','Synthetic','private','{}');
INSERT INTO sources VALUES ('collection','source','import',NULL,'synthetic','{}');
INSERT INTO documents VALUES ('document','collection','source','page.md');
INSERT INTO chunk_sets VALUES ('set','collection','mapped-structural-chunks/4',
'synthetic-counter/1','building',NULL);",
            )?;
            tx.execute(
                "INSERT INTO revisions(
                id,document_id,original_digest,canonical_digest,status,metadata_json)
                VALUES ('revision','document',?1,?1,'valid','{}')",
                [original.as_str()],
            )?;
            tx.execute(
                "INSERT INTO chunks VALUES ('set','chunk-rows','revision',NULL,?1,12,0,37)",
                [input.as_str()],
            )?;
            Ok(())
        })
        .unwrap();
        db
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
/// Typed golden graph.
pub(super) fn graph() -> DeliveryGraph {
    DeliveryGraph::from_bytes(
        include_bytes!("../../../tests/fixtures/unit-graph-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap()
}
/// Stores graph and separately versioned mapping bytes, without pins.
pub(super) fn put(db: &Database, graph: &DeliveryGraph) -> Digest {
    db.put(
        include_bytes!("../../../tests/fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
        "application/json",
    )
    .unwrap();
    db.put(&graph.to_bytes().unwrap(), "application/json")
        .unwrap()
}
/// Counts graph records without authorization (test oracle only).
pub(super) fn count(db: &Database) -> i64 {
    db.reader()
        .unwrap()
        .query_row("SELECT count(*) FROM revision_unit_graphs", [], |row| {
            row.get(0)
        })
        .unwrap()
}
/// Records an additional hidden source and revision with no graph.
pub(super) fn hidden(db: &Database) {
    db.write::<_, Error>(|tx| {
        tx.execute_batch(
            "INSERT INTO sources VALUES ('collection','hidden','import',NULL,'hidden','{}');
            INSERT INTO documents VALUES ('hidden-document','collection','hidden','hidden.md');",
        )?;
        tx.execute(
            "INSERT INTO revisions(
                id,document_id,original_digest,canonical_digest,status,metadata_json)
                VALUES ('hidden-revision','hidden-document',?1,?1,'valid','{}')",
            params![graph().descriptor.original_markdown_digest.as_str()],
        )?;
        Ok(())
    })
    .unwrap();
}
/// One source grant, deliberately not collection-wide.
pub(super) fn scoped() -> ScopeSet {
    ScopeSet::new(
        ["workspace/default/collection/collection/source/source"
            .parse()
            .unwrap()]
        .into(),
    )
}
