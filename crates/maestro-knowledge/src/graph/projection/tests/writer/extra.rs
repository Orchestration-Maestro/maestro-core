//! Additional validation cases for the generic projection writer.

use super::super::contract::{edge, fact};
use super::*;
use maestro_kernel::facts::{EntityKind, EntityName, Predicate};

#[test]
fn writer_rejects_wrong_generation_and_denied_scope_before_backend_writes() {
    let scope = ProjectionScope {
        collection_id: "c".to_owned(),
        generation_id: 12,
    };
    let (path, database, scopes) = scoped();
    let mut backend = Fake::default();
    let mut writer = ProjectionWriter::create(&mut backend, scope.clone()).unwrap();
    assert!(
        writer
            .write_batch(&scopes, &[edge(13, EdgeFamily::KnowledgeClaim)], &[])
            .is_err()
    );
    let denied = database.visible("no-grants").unwrap();
    assert!(matches!(
        writer.write_batch(&denied, &[edge(12, EdgeFamily::KnowledgeClaim)], &[]),
        Err(ProjectionError::Unauthorized)
    ));
    drop(writer);
    drop(database);
    fs::remove_dir_all(path).unwrap();
    assert!(backend.pending.get(&12).is_none_or(Vec::is_empty));
}

#[test]
fn writer_rejects_a_fact_from_another_collection() {
    let scope = ProjectionScope {
        collection_id: "c".to_owned(),
        generation_id: 14,
    };
    let (path, database, scopes) = scoped();
    let mut invalid = fact(&scope);
    invalid.claim.collection_id = "other".to_owned();
    let mut backend = Fake::default();
    let mut writer = ProjectionWriter::create(&mut backend, scope).unwrap();
    assert!(writer.write_batch(&scopes, &[], &[invalid]).is_err());
    drop(writer);
    drop(database);
    fs::remove_dir_all(path).unwrap();
    assert_eq!(backend.fact_counts.get(&14), None);
}

#[test]
fn schema_and_required_indexes_are_checked_independently_of_expected_values() {
    for backend in [
        Fake {
            schema: Some("unknown-schema".to_owned()),
            ..Fake::default()
        },
        Fake {
            omit_edge_index: true,
            ..Fake::default()
        },
    ] {
        let scope = ProjectionScope {
            collection_id: "c".to_owned(),
            generation_id: 15,
        };
        let mut backend = backend;
        let mut writer = ProjectionWriter::create(&mut backend, scope.clone()).unwrap();
        let expected = writer.backend.verify_unpublished(&scope).unwrap();
        assert!(
            writer
                .verify_and_publish(&expected, (&Digest::of(b"set"), 1))
                .is_err()
        );
    }
}

#[test]
fn an_entity_claim_cannot_be_written_as_a_literal_subject_fact() {
    use maestro_kernel::facts::Object;

    let scope = ProjectionScope {
        collection_id: "c".to_owned(),
        generation_id: 6,
    };
    let (path, database, all) = scoped();
    let mut catalog_claim = fact(&scope);
    catalog_claim.claim.claim.object = Object::Entity(EntityName {
        kind: EntityKind::Component,
        name: "engine".to_owned(),
    });
    let mut backend = Fake::default();
    let mut writer = ProjectionWriter::create(&mut backend, scope).unwrap();
    assert!(writer.write_batch(&all, &[], &[catalog_claim]).is_err());
    drop(writer);
    drop(database);
    fs::remove_dir_all(path).unwrap();
    assert!(backend.pending.get(&6).is_none_or(Vec::is_empty));
    assert_eq!(backend.fact_counts.get(&6), None);
}

#[test]
fn a_literal_object_cannot_use_an_entity_predicate_as_a_fact() {
    let scope = ProjectionScope {
        collection_id: "c".to_owned(),
        generation_id: 7,
    };
    let (path, database, all) = scoped();
    let mut invalid = fact(&scope);
    invalid.claim.claim.predicate = Predicate::Requires;
    let mut backend = Fake::default();
    let mut writer = ProjectionWriter::create(&mut backend, scope).unwrap();
    assert!(writer.write_batch(&all, &[], &[invalid]).is_err());
    drop(writer);
    drop(database);
    fs::remove_dir_all(path).unwrap();
    assert_eq!(backend.fact_counts.get(&7), None);
}

#[test]
fn an_unverified_count_or_index_set_never_becomes_ready() {
    let scope = ProjectionScope {
        collection_id: "c".to_owned(),
        generation_id: 5,
    };
    let (path, database, _scopes) = scoped();
    let mut backend = Fake::default();
    let mut writer = ProjectionWriter::create(&mut backend, scope.clone()).unwrap();
    let wrong = BuildVerification {
        schema: "maestro-typed-edges/2".to_owned(),
        family_counts: BTreeMap::new(),
        fact_count: 0,
        content_digest: content::digest(&[], &[]).unwrap(),
        indexes: BTreeSet::new(),
    };
    assert!(
        writer
            .verify_and_publish(&wrong, (&Digest::of(b"set"), 1))
            .is_err()
    );
    drop(writer);
    assert!(
        !backend
            .published
            .iter()
            .any(|(published_scope, _)| published_scope.generation_id == 5)
    );
    drop(database);
    fs::remove_dir_all(path).unwrap();
}
