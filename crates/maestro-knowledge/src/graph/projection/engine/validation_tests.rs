//! Native corruption and storage-shape refusals, independent of writer authorization.

use super::{
    rows, schema,
    tests::{Fixture, edge, full_fact, scope},
    transaction::Transactions,
};
use crate::graph::projection::{EntityFact, ProjectionEdge, content};
use lbug::{Connection, LogicalType, Value};
use maestro_kernel::{
    artifact::Digest,
    facts::{EntityName, LiteralKind, Object, Predicate},
};
use std::slice;

#[test]
fn native_catalog_refuses_schema_scope_and_each_access_path_mismatch() {
    let fixture = Fixture::new();
    let database = fixture.writer();
    let connection = Connection::new(&database).unwrap();
    schema::create(&connection, &scope()).unwrap();
    assert_eq!(schema::verify(&connection, &scope()).unwrap().len(), 2);
    for queries in [
        vec![
            "MATCH (p:Projection) DELETE p",
            "CREATE (:Projection {schema: 'wrong', collection: 'c', generation: 1})",
        ],
        vec!["MATCH (p:Projection) SET p.collection = 'other'"],
        vec!["MATCH (p:Projection) SET p.generation = 2"],
        vec!["MATCH (p:Projection) DELETE p"],
        vec!["CREATE NODE TABLE Extra(id STRING, PRIMARY KEY(id))"],
        vec!["ALTER TABLE Entity ADD extra STRING"],
        vec!["ALTER TABLE Projection ADD extra STRING"],
        vec!["ALTER TABLE Edge ADD extra STRING"],
        vec![
            "DROP TABLE Edge",
            "DROP TABLE Entity",
            "CREATE NODE TABLE Entity(id STRING, facts BLOB[], other STRING, PRIMARY KEY(other))",
            "CREATE REL TABLE Edge(FROM Entity TO Entity, id STRING, family STRING,
                relation STRING, collection STRING, generation INT64)",
        ],
        vec![
            "DROP TABLE Edge",
            "CREATE REL TABLE Edge(FROM Projection TO Entity, id STRING, family STRING,
                relation STRING, collection STRING, generation INT64)",
        ],
        vec![
            "DROP TABLE Edge",
            "CREATE REL TABLE Edge(FROM Entity TO Entity, id STRING, family STRING,
                relation STRING, collection STRING, generation INT64)
                WITH (storage_direction = 'fwd')",
        ],
    ] {
        connection.query("BEGIN TRANSACTION").unwrap();
        for query in &queries {
            connection.query(query).unwrap();
        }
        assert!(
            schema::verify(&connection, &scope()).is_err(),
            "accepted {queries:?}"
        );
        connection.query("ROLLBACK").unwrap();
        assert_eq!(schema::verify(&connection, &scope()).unwrap().len(), 2);
    }
    assert!(schema::create(&connection, &scope()).is_err());
    assert_eq!(schema::verify(&connection, &scope()).unwrap().len(), 2);
}

#[test]
fn native_rows_refuse_duplicate_malformed_and_misattached_durable_content() {
    let fixture = Fixture::new();
    let database = fixture.writer();
    let connection = Connection::new(&database).unwrap();
    schema::create(&connection, &scope()).unwrap();
    let mut tx = Transactions::default();
    tx.write_batch(&connection, &scope(), &[edge()], &[full_fact()])
        .unwrap();
    let baseline = rows::read(&connection, &scope())
        .unwrap()
        .verification()
        .unwrap();
    for query in [
        "MATCH (s:Entity)-[e:Edge]->(t:Entity)
            CREATE (s)-[:Edge {id:e.id, family:e.family, relation:e.relation,
                collection:e.collection, generation:e.generation}]->(t)",
        "MATCH ()-[e:Edge]->() SET e.family = 'unknown'",
        "MATCH ()-[e:Edge]->() SET e.relation = 'ALIAS_OF'",
        "MATCH ()-[e:Edge]->() SET e.relation = ''",
        "MATCH ()-[e:Edge]->() SET e.generation = 2",
        "MATCH ()-[e:Edge]->() SET e.collection = 'other'",
        "MATCH ()-[e:Edge]->() SET e.id = 'malformed'",
        "MATCH ()-[e:Edge]->() SET e.id = null",
        "MATCH (e:Entity) WHERE size(e.facts) > 0 SET e.facts = list_concat(e.facts, e.facts)",
        "MATCH (e:Entity) WHERE size(e.facts) > 0 SET e.facts = null",
        "MATCH (s:Entity), (t:Entity) WHERE size(s.facts) > 0 AND size(t.facts) = 0
            SET t.facts = s.facts",
    ] {
        connection.query("BEGIN TRANSACTION").unwrap();
        connection.query(query).unwrap();
        assert!(
            rows::read(&connection, &scope()).is_err(),
            "accepted {query}"
        );
        connection.query("ROLLBACK").unwrap();
        assert_eq!(
            rows::read(&connection, &scope())
                .unwrap()
                .verification()
                .unwrap(),
            baseline
        );
    }
    let mut statement = connection
        .prepare("CREATE (:Entity {id: $id, facts: $facts})")
        .unwrap();
    for (id, facts) in [
        (
            Digest::of(b"orphan").as_str().to_owned(),
            Value::List(LogicalType::Blob, vec![]),
        ),
        ("invalid ID".into(), Value::List(LogicalType::Blob, vec![])),
        (
            Digest::of(b"bad-bytes").as_str().to_owned(),
            Value::List(LogicalType::Blob, vec![Value::Blob(vec![0])]),
        ),
    ] {
        connection.query("BEGIN TRANSACTION").unwrap();
        connection
            .execute(
                &mut statement,
                vec![("id", Value::String(id)), ("facts", facts)],
            )
            .unwrap();
        assert!(rows::read(&connection, &scope()).is_err());
        connection.query("ROLLBACK").unwrap();
    }
}

#[test]
fn native_batch_shape_refusals_have_valid_neighbours_and_no_partial_rows() {
    let fixture = Fixture::new();
    let database = fixture.writer();
    let connection = Connection::new(&database).unwrap();
    schema::create(&connection, &scope()).unwrap();
    let mut tx = Transactions::default();
    tx.write_batch(&connection, &scope(), &[edge()], &[full_fact()])
        .unwrap();
    let baseline = rows::read(&connection, &scope())
        .unwrap()
        .verification()
        .unwrap();
    let mut fresh = edge();
    fresh.id = Digest::of(b"fresh");
    let mut wrong_scope = scope();
    wrong_scope.collection_id = "other".into();
    assert!(tx.write_batch(&connection, &wrong_scope, &[], &[]).is_err());
    for (edges, facts) in [
        (vec![fresh.clone(), fresh.clone()], vec![]),
        (vec![fresh.clone(), edge()], vec![]),
        (
            vec![fresh.clone()],
            vec![EntityFact {
                claim: full_fact().claim,
                ..content::tests::fact()
            }],
        ),
    ] {
        assert!(
            tx.write_batch(&connection, &scope(), &edges, &facts)
                .is_err()
        );
        assert_eq!(
            rows::read(&connection, &scope())
                .unwrap()
                .verification()
                .unwrap(),
            baseline
        );
    }
    let mutations: [fn(&mut EntityFact); 5] = [
        |fact: &mut EntityFact| {
            fact.scope.generation_id += 1;
        },
        |fact: &mut EntityFact| {
            fact.claim.claim.predicate = Predicate::Requires;
        },
        |fact: &mut EntityFact| {
            fact.claim.claim.object = Object::Entity(EntityName {
                kind: fact.claim.claim.subject.kind,
                name: "other".into(),
            });
        },
        |fact: &mut EntityFact| {
            fact.claim.claim.supports[0].span.end = 2;
        },
        |fact: &mut EntityFact| {
            fact.claim
                .claim
                .supports
                .push(fact.claim.claim.supports[0].clone());
        },
    ];
    for mutate in mutations {
        let mut fact = full_fact();
        fact.claim.id = Digest::of(b"fresh-fact");
        mutate(&mut fact);
        assert!(
            tx.write_batch(&connection, &scope(), slice::from_ref(&fresh), &[fact])
                .is_err()
        );
        assert_eq!(
            rows::read(&connection, &scope())
                .unwrap()
                .verification()
                .unwrap(),
            baseline
        );
    }
    assert_valid_neighbours(&connection, &mut tx, fresh);
}

/// Literal spellings and a self-loop are accepted beside the malformed cases.
fn assert_valid_neighbours(
    connection: &Connection<'_>,
    tx: &mut Transactions,
    fresh: ProjectionEdge,
) {
    for (kind, lexeme) in [
        (LiteralKind::Boolean, "false"),
        (LiteralKind::Integer, "-001"),
        (LiteralKind::Decimal, "-01.20"),
    ] {
        let mut fact = full_fact();
        fact.claim.id = Digest::of(lexeme.as_bytes());
        if let Object::Literal(literal) = &mut fact.claim.claim.object {
            literal.kind = kind;
            literal.lexeme = lexeme.into();
        }
        tx.write_batch(connection, &scope(), &[], slice::from_ref(&fact))
            .unwrap();
        assert!(
            rows::read(connection, &scope())
                .unwrap()
                .facts
                .contains(&fact)
        );
    }
    tx.write_batch(
        connection,
        &scope(),
        &[ProjectionEdge {
            id: Digest::of(b"self-loop"),
            target: fresh.source.clone(),
            ..fresh
        }],
        &[],
    )
    .unwrap();
    assert_eq!(rows::read(connection, &scope()).unwrap().edges.len(), 2);
}
