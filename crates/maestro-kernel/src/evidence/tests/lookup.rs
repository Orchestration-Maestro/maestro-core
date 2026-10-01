use super::support::{Scratch, chunk, granted, metadata, span_of};
use crate::{
    evidence::{ChunkLocation, SectionLocation},
    generation::NewGeneration,
    scope::ScopeSet,
};
use std::slice;

#[test]
fn chunk_locations_require_readable_published_membership_and_pin_selectors() {
    let scratch = Scratch::new();
    let database = scratch.open(metadata(None));
    let chunk_id = "chunk-a";
    chunk(
        &database,
        chunk_id,
        Some("sec-a"),
        span_of("The agent listens"),
    );
    let manifest = database.put(b"{}", "application/json").unwrap();
    database.complete_chunk_set("set-a", &manifest).unwrap();
    let generation = database
        .create_generation(&NewGeneration {
            collection_id: "ctm".to_owned(),
            chunk_set_id: "set-a".to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap();
    let verified = database
        .create_generation(&NewGeneration {
            collection_id: "ctm".to_owned(),
            chunk_set_id: "set-a".to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap();
    database.verify_generation(verified.id, 1).unwrap();
    let failed = database
        .create_generation(&NewGeneration {
            collection_id: "ctm".to_owned(),
            chunk_set_id: "set-a".to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap();
    database.verify_generation(failed.id, 1).unwrap();
    database.fail_generation(failed.id).unwrap();
    let source_scopes = granted(
        &database,
        "source-reader",
        "workspace/default/collection/ctm/source/docs",
    );
    let expected = ChunkLocation {
        collection_id: "ctm".to_owned(),
        generation_id: generation.id,
        chunk_set_id: "set-a".to_owned(),
        title: Some("Installing the agent".to_owned()),
    };

    assert!(
        database
            .chunk_locations(&source_scopes, chunk_id, None, None)
            .unwrap()
            .is_empty(),
        "building, verified, and failed generations are not retrievable"
    );
    for ineligible in [verified.id, failed.id] {
        assert!(
            database
                .chunk_locations(&source_scopes, chunk_id, Some("ctm"), Some(ineligible))
                .unwrap()
                .is_empty(),
            "generation {ineligible} is not published or retired"
        );
    }
    database.verify_generation(generation.id, 1).unwrap();
    database.publish_generation(generation.id).unwrap();
    let published = database
        .chunk_locations(&source_scopes, chunk_id, None, None)
        .unwrap();
    assert_eq!(published.as_slice(), slice::from_ref(&expected));
    assert_eq!(
        database
            .chunk_locations(&source_scopes, chunk_id, Some("ctm"), Some(generation.id))
            .unwrap(),
        [expected]
    );
    assert!(
        database
            .chunk_locations(&ScopeSet::default_workspace(), "missing", None, None)
            .unwrap()
            .is_empty()
    );
    let no_read = database.visible("nobody").unwrap();
    assert!(
        database
            .chunk_locations(&no_read, chunk_id, None, None)
            .unwrap()
            .is_empty()
    );
    assert!(
        database
            .chunk_locations(&source_scopes, chunk_id, Some("other"), Some(generation.id))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn section_locations_deduplicate_sections_spanning_multiple_chunks() {
    let scratch = Scratch::new();
    let database = scratch.open(metadata(None));
    chunk(
        &database,
        "chunk-a",
        Some("sec-a"),
        span_of("The agent listens"),
    );
    chunk(
        &database,
        "chunk-b",
        Some("sec-a"),
        span_of("The default port is 7006"),
    );
    let manifest = database.put(b"{}", "application/json").unwrap();
    database.complete_chunk_set("set-a", &manifest).unwrap();
    let generation = database
        .create_generation(&NewGeneration {
            collection_id: "ctm".to_owned(),
            chunk_set_id: "set-a".to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap();
    let source_scopes = granted(
        &database,
        "source-reader",
        "workspace/default/collection/ctm/source/docs",
    );
    let expected = SectionLocation {
        collection_id: "ctm".to_owned(),
        generation_id: generation.id,
        chunk_set_id: "set-a".to_owned(),
    };

    assert!(
        database
            .section_locations(&source_scopes, "sec-a", None, None)
            .unwrap()
            .is_empty()
    );
    database.verify_generation(generation.id, 2).unwrap();
    database.publish_generation(generation.id).unwrap();
    let published = database
        .section_locations(&source_scopes, "sec-a", None, None)
        .unwrap();
    assert_eq!(published.as_slice(), slice::from_ref(&expected));
    let collection_pinned = database
        .section_locations(&source_scopes, "sec-a", Some("ctm"), None)
        .unwrap();
    assert_eq!(collection_pinned.as_slice(), slice::from_ref(&expected));
    let replacement = database
        .create_generation(&NewGeneration {
            collection_id: "ctm".to_owned(),
            chunk_set_id: "set-a".to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap();
    database.verify_generation(replacement.id, 2).unwrap();
    database.publish_generation(replacement.id).unwrap();
    let current = database
        .section_locations(&source_scopes, "sec-a", None, None)
        .unwrap();
    assert_eq!(
        current.as_slice(),
        slice::from_ref(&SectionLocation {
            collection_id: "ctm".to_owned(),
            generation_id: replacement.id,
            chunk_set_id: "set-a".to_owned(),
        })
    );
    assert_eq!(
        database
            .section_locations(&source_scopes, "sec-a", Some("ctm"), Some(generation.id))
            .unwrap(),
        [expected]
    );
    let no_read = database.visible("nobody").unwrap();
    assert!(
        database
            .section_locations(&no_read, "sec-a", None, None)
            .unwrap()
            .is_empty()
    );
    assert!(
        database
            .section_locations(&source_scopes, "sec-a", Some("other"), Some(generation.id))
            .unwrap()
            .is_empty()
    );
}
