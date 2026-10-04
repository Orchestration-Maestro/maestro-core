//! Native schema, rows and transaction contract tests.

#[cfg(windows)]
use super::schema::tests::install_reader_fixture;
use super::{
    open::{open, tests::Scratch},
    rows,
};
#[cfg(not(windows))]
use super::{schema, transaction::Transactions};
use crate::graph::projection::tests::contract;
use crate::graph::projection::{EdgeFamily, EntityFact, ProjectionEdge, ProjectionScope, content};
use lbug::{Connection, Database, RootDirectory, SystemConfig, Value};
use maestro_kernel::{
    artifact::Digest,
    evidence::Span,
    facts::{Object, ReviewState, Support, Validity},
};
use std::path::PathBuf;
#[cfg(not(windows))]
use std::slice;

/// A private owned native test root.
pub(super) struct Fixture {
    /// Directory removed after all native handles drop.
    pub(super) path: PathBuf,
    /// Root capability held throughout the test.
    pub(super) root: RootDirectory,
    /// Fields drop in declaration order: release the root before deleting its directory.
    _scratch: Scratch,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let scratch = Scratch::new();
        let path = scratch.0.clone();
        let root = RootDirectory::open(&path).unwrap();
        Self {
            path,
            root,
            _scratch: scratch,
        }
    }
    pub(super) fn writer(&self) -> Database {
        #[cfg(windows)]
        #[expect(
            clippy::disallowed_methods,
            reason = "immutable Windows reader fixture only; rooted writers refuse"
        )]
        let database = Database::new(self.path.join("rows.lbdb"), config()).unwrap();
        #[cfg(not(windows))]
        let database = open(&self.root, "rows.lbdb", config()).unwrap();
        database
    }
    pub(super) fn reader(&self) -> Database {
        #[cfg(windows)]
        super::open::tests::private_windows_fixture(&self.path);
        open(&self.root, "rows.lbdb", config().read_only(true)).unwrap()
    }
}
#[test]
fn fixture_cleanup_releases_root_before_removing_directory() {
    let fixture = Fixture::new();
    let path = fixture.path.clone();
    // No Database or Connection exists: the fixture's root alone must be released.
    drop(fixture);
    assert!(!path.exists());
}

pub(super) fn config() -> SystemConfig {
    super::open::tests::config()
}
pub(super) fn scope() -> ProjectionScope {
    ProjectionScope {
        collection_id: "c".into(),
        generation_id: 1,
    }
}
pub(super) fn edge() -> ProjectionEdge {
    ProjectionEdge {
        id: Digest::of(b"edge-id"),
        scope: scope(),
        family: EdgeFamily::KnowledgeClaim,
        source: Digest::of(b"source"),
        target: Digest::of(b"target"),
        relation: "REQUIRES".into(),
    }
}
pub(super) fn full_fact() -> EntityFact {
    let mut fact = content::tests::fact();
    fact.claim.claim.subject.name = "mode '雪' \\ \0".into();
    if let Object::Literal(literal) = &mut fact.claim.claim.object {
        literal.lexeme = "fast '雪' \\ \0".into();
    }
    fact.claim
        .claim
        .conditions
        .insert("a'雪".into(), "value\\\0".into());
    fact.claim.claim.conditions.insert("z".into(), "[]".into());
    fact.claim.claim.version = Validity::Bounded {
        start: Some(String::new()),
        end: None,
    };
    fact.claim.claim.world = Validity::Bounded {
        start: None,
        end: Some("2030-01-01".into()),
    };
    fact.claim.claim.supports = vec![Support {
        revision_id: "r'雪".into(),
        block_id: "b\\".into(),
        span: Span { start: 2, end: 7 },
        quote_digest: Digest::of(b"quote"),
    }];
    fact.claim.review = ReviewState::Flagged;
    fact
}

#[test]
fn fact_codec_is_full_canonical_and_rejects_malformed_bytes() {
    for fact in [content::tests::fact(), full_fact()] {
        let encoded = rows::encode_fact(&fact).unwrap();
        assert_eq!(rows::decode_fact(&encoded).unwrap(), fact);
        for end in 0..encoded.len() {
            assert!(rows::decode_fact(&encoded[..end]).is_err());
        }
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(rows::decode_fact(&trailing).is_err());
        let mut version = encoded;
        version[0] ^= 1;
        assert!(rows::decode_fact(&version).is_err());
    }
    assert_eq!(
        content::digest(&[], &[full_fact()]).unwrap().as_str(),
        "ed026d40f40be88719aee1b522801ad27879865403b65fa09bffa1929dffd0fe"
    );
    assert_eq!(
        content::digest(&[], &[content::tests::fact()])
            .unwrap()
            .as_str(),
        "ade1c37f3c52bdccb5fe3d585b56a9c53a0162088e38c88aed338f61894d9ea3"
    );
}

#[test]
fn native_bound_full_rows_survive_close_and_catalog_verification() {
    let fixture = Fixture::new();
    let expected_edges = vec![
        edge(),
        ProjectionEdge {
            id: Digest::of(b"catalog"),
            family: EdgeFamily::CatalogDependency,
            relation: "depends_on '雪' \\".into(),
            ..edge()
        },
    ];
    let mut expected_facts = vec![
        full_fact(),
        EntityFact {
            claim: content::tests::fact().claim,
            ..full_fact()
        },
    ];
    // Distinct IDs on one subject exercise append rather than replacement.
    expected_facts[1].claim.id = Digest::of(b"second-fact");
    {
        let db = fixture.writer();
        let conn = Connection::new(&db).unwrap();
        #[cfg(not(windows))]
        {
            schema::create(&conn, &scope(), &contract::pins(), 1).unwrap();
            let mut tx = Transactions::default();
            for fact in &expected_facts {
                tx.write_batch(&conn, &scope(), &[], slice::from_ref(fact))
                    .unwrap();
            }
            tx.write_batch(&conn, &scope(), &expected_edges, &[])
                .unwrap();
        }
        #[cfg(windows)]
        {
            install_reader_fixture(&conn, &scope(), &contract::pins());
            super::transaction::tests::populate_reader_fixture(
                &conn,
                &scope(),
                &expected_edges,
                &expected_facts,
            );
        }
        conn.query("CHECKPOINT").unwrap();
    }
    let db = fixture.reader();
    let conn = Connection::new(&db).unwrap();
    let found = rows::read(&conn, &scope()).unwrap();
    let mut expected_edges = expected_edges;
    expected_edges.sort_by(|left, right| left.id.cmp(&right.id));
    assert_eq!(found.edges, expected_edges);
    expected_facts.sort_by(|left, right| left.claim.id.cmp(&right.claim.id));
    assert_eq!(found.facts, expected_facts);
    let verification = found.verification().unwrap();
    assert_eq!(verification.indexes.len(), 2);
    assert_eq!(verification.fact_count, 2);
    assert_eq!(verification.family_counts[&EdgeFamily::KnowledgeClaim], 1);
    assert_eq!(
        verification.family_counts[&EdgeFamily::CatalogDependency],
        1
    );
    assert_eq!(
        conn.query("MATCH (e:Entity) RETURN count(e)")
            .unwrap()
            .next()
            .unwrap(),
        [Value::Int64(3)]
    );
}

#[cfg(not(windows))]
mod writes {
    use super::*;
    use maestro_kernel::facts::LiteralKind;
    #[test]
    fn native_failure_is_after_write_one_shot_and_rolls_back_edges_and_facts() {
        let fixture = Fixture::new();
        let mut tx = Transactions::default();
        {
            let db = fixture.writer();
            let conn = Connection::new(&db).unwrap();
            schema::create(&conn, &scope(), &contract::pins(), 1).unwrap();
            tx.inject_batch_failure().unwrap();
            tx.write_batch(&conn, &scope(), &[], &[]).unwrap();
            assert!(
                tx.write_batch(&conn, &scope(), &[edge()], &[full_fact()])
                    .is_err()
            );
            assert_eq!(tx.mutations_before_failure(), 1);
            assert!(rows::read(&conn, &scope()).unwrap().edges.is_empty());
            assert!(rows::read(&conn, &scope()).unwrap().facts.is_empty());
            assert_eq!(
                conn.query("MATCH (e:Entity) RETURN count(e)")
                    .unwrap()
                    .next()
                    .unwrap(),
                [Value::Int64(0)]
            );
            conn.query("CHECKPOINT").unwrap();
        }
        {
            let db = fixture.reader();
            let conn = Connection::new(&db).unwrap();
            assert_eq!(
                rows::read(&conn, &scope())
                    .unwrap()
                    .verification()
                    .unwrap()
                    .content_digest,
                content::digest(&[], &[]).unwrap()
            );
        }
        let db = fixture.writer();
        let conn = Connection::new(&db).unwrap();
        tx.write_batch(&conn, &scope(), &[edge()], &[full_fact()])
            .unwrap();
        assert_eq!(rows::read(&conn, &scope()).unwrap().facts, [full_fact()]);
        assert_eq!(
            rows::read(&conn, &scope())
                .unwrap()
                .verification()
                .unwrap()
                .content_digest
                .as_str(),
            "c6cbe83e1fc08ba1416a1a8dea6316083e06c55886e10bed1fab59b3406d8bbd"
        );
        tx.inject_batch_failure().unwrap();
        let other = EntityFact {
            claim: content::tests::fact().claim,
            ..full_fact()
        };
        let mut other = other;
        other.claim.id = Digest::of(b"another");
        assert!(
            tx.write_batch(&conn, &scope(), &[], slice::from_ref(&other))
                .is_err()
        );
        tx.write_batch(&conn, &scope(), &[], &[other]).unwrap();
        assert_eq!(rows::read(&conn, &scope()).unwrap().facts.len(), 2);
    }

    #[test]
    fn native_refuses_scope_duplicates_and_malformed_rows_without_partial_writes() {
        let fixture = Fixture::new();
        let db = fixture.writer();
        let conn = Connection::new(&db).unwrap();
        schema::create(&conn, &scope(), &contract::pins(), 1).unwrap();
        let mut tx = Transactions::default();
        tx.write_batch(&conn, &scope(), &[edge()], &[full_fact()])
            .unwrap();
        let before = rows::read(&conn, &scope()).unwrap().verification().unwrap();
        let mut invalid_edge = edge();
        invalid_edge.id = Digest::of(b"fresh");
        invalid_edge.relation = "DEFAULTS_TO".into();
        let mut wrong_scope = edge();
        wrong_scope.scope.generation_id += 1;
        let mut cross_kind = edge();
        cross_kind.id = full_fact().claim.id;
        let mut invalid_fact = full_fact();
        invalid_fact.claim.id = Digest::of(b"fresh-fact");
        invalid_fact.claim.collection_id = "other".into();
        let mut invalid_literal = full_fact();
        invalid_literal.claim.id = Digest::of(b"literal");
        if let Object::Literal(literal) = &mut invalid_literal.claim.claim.object {
            literal.kind = LiteralKind::Boolean;
        }
        for (edges, facts) in [
            (vec![edge()], vec![]),
            (vec![invalid_edge], vec![]),
            (vec![wrong_scope], vec![]),
            (vec![cross_kind], vec![]),
            (vec![], vec![full_fact()]),
            (vec![], vec![invalid_fact]),
            (vec![], vec![invalid_literal]),
        ] {
            assert!(tx.write_batch(&conn, &scope(), &edges, &facts).is_err());
            assert_eq!(
                rows::read(&conn, &scope()).unwrap().verification().unwrap(),
                before
            );
        }
        assert!(
            rows::read(
                &conn,
                &ProjectionScope {
                    generation_id: 2,
                    ..scope()
                }
            )
            .is_err()
        );
    }
}

#[test]
fn empty_native_rows_have_independently_frozen_digest_and_zero_counts() {
    let fixture = Fixture::new();
    {
        let database = fixture.writer();
        let connection = Connection::new(&database).unwrap();
        #[cfg(not(windows))]
        schema::create(&connection, &scope(), &contract::pins(), 1).unwrap();
        #[cfg(windows)]
        install_reader_fixture(&connection, &scope(), &contract::pins());
        connection.query("CHECKPOINT").unwrap();
    }
    let database = fixture.reader();
    let connection = Connection::new(&database).unwrap();
    let found = rows::read(&connection, &scope())
        .unwrap()
        .verification()
        .unwrap();
    assert_eq!(found.schema, "maestro-typed-edges/3");
    assert_eq!(
        found.content_digest.as_str(),
        "a3b2abe4237b84ac08917ce65e45f14ec2b9df6cd0426c18d27225c53e742a35"
    );
    assert!(found.family_counts.is_empty());
    assert_eq!(found.fact_count, 0);
    assert_eq!(found.indexes.len(), 2);
}
