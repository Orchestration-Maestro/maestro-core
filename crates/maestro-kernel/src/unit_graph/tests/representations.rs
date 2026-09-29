//! Revision shards complete frozen representation sets and bind once.
use super::support::{Scratch, graph, hidden, put, scoped};
use crate::{
    artifact::Digest,
    generation::NewGeneration,
    representation::{
        RepresentationKey, RepresentationLayout, RepresentationMember, RepresentationSetSpec,
        RepresentationShard, RepresentationState,
    },
    scope::ScopeSet,
    store::Error as StoreError,
    unit_graph::DeliveryGraph,
};

#[test]
fn representation_shards_begin_repeat_conflict_and_fail() {
    let scratch = Scratch::new();
    let db = scratch.open();
    let graph = graph();
    let graph_digest = put(&db, &graph);
    db.record_revision_graph(&scoped(), "set", &graph_digest)
        .unwrap();
    let scope = ScopeSet::default_workspace();
    let generation = db
        .create_generation(&NewGeneration {
            collection_id: "collection".into(),
            chunk_set_id: "set".into(),
            embedding_profile: "dense".into(),
            sparse_profile: "sparse".into(),
        })
        .unwrap();
    assert!(
        db.generation_representation(&scope, generation.id)
            .unwrap()
            .is_none()
    );

    let key = RepresentationKey {
        collection_id: "collection",
        chunk_set_id: "set",
        id: "rep",
    };
    let initial_spec = spec("rep", graph.descriptor.profile_digest.clone());
    db.begin_representation_set(&scope, &initial_spec).unwrap();
    db.begin_representation_set(&scope, &initial_spec).unwrap();
    let conflict = RepresentationSetSpec {
        profile_digest: Digest::of(b"different"),
        ..initial_spec.clone()
    };
    assert!(db.begin_representation_set(&scope, &conflict).is_err());

    let incomplete = shard(&graph, &graph_digest, "rep", 1);
    let digest = db
        .put(&incomplete.to_bytes().unwrap(), "application/json")
        .unwrap();
    assert!(
        db.record_representation_revision(&scope, &key, &digest)
            .is_err()
    );
    db.fail_representation_set(&scope, &key).unwrap();
    assert!(db.complete_representation_set(&scope, &key).is_err());
}

#[test]
fn representation_shards_complete_and_bind_generation_once() {
    let scratch = Scratch::new();
    let db = scratch.open();
    let graph = graph();
    let graph_digest = put(&db, &graph);
    db.record_revision_graph(&scoped(), "set", &graph_digest)
        .unwrap();
    let scope = ScopeSet::default_workspace();
    let generation = db
        .create_generation(&NewGeneration {
            collection_id: "collection".into(),
            chunk_set_id: "set".into(),
            embedding_profile: "dense".into(),
            sparse_profile: "sparse".into(),
        })
        .unwrap();
    let key = RepresentationKey {
        collection_id: "collection",
        chunk_set_id: "set",
        id: "complete",
    };
    let spec = spec("complete", graph.descriptor.profile_digest.clone());
    db.begin_representation_set(&scope, &spec).unwrap();
    let complete_shard = shard(&graph, &graph_digest, "complete", 3);
    let mut bad_shard = complete_shard.clone();
    bad_shard.members[0].input_digest = Digest::of(b"wrong input");
    let bad_digest = db
        .put(&bad_shard.to_bytes().unwrap(), "application/json")
        .unwrap();
    assert!(
        db.record_representation_revision(&scope, &key, &bad_digest)
            .is_err()
    );
    let shard_digest = db
        .put(&complete_shard.to_bytes().unwrap(), "application/json")
        .unwrap();
    db.record_representation_revision(&scope, &key, &shard_digest)
        .unwrap();
    db.record_representation_revision(&scope, &key, &shard_digest)
        .unwrap();
    assert_eq!(
        db.representation_set(&scope, &key).unwrap().unwrap().state,
        RepresentationState::Building
    );
    assert!(
        db.representation_shard(&scope, &key, "revision")
            .unwrap()
            .is_some()
    );
    assert!(db.complete_representation_set(&scope, &key).is_err());
    db.complete_chunk_set("set", &graph_digest).unwrap();
    db.complete_representation_set(&scope, &key).unwrap();
    assert_eq!(
        db.representation_set(&scope, &key).unwrap().unwrap().state,
        RepresentationState::Complete
    );
    assert!(
        db.bind_generation_representation(&scoped(), generation.id, &key)
            .is_err()
    );
    db.bind_generation_representation(&scope, generation.id, &key)
        .unwrap();
    assert!(
        db.bind_generation_representation(&scope, generation.id, &key)
            .is_err()
    );
    assert_eq!(
        db.generation_representation(&scope, generation.id)
            .unwrap()
            .unwrap()
            .id,
        "complete"
    );
    let foreign = db
        .create_generation(&NewGeneration {
            collection_id: "collection".into(),
            chunk_set_id: "set".into(),
            embedding_profile: "other".into(),
            sparse_profile: "sparse".into(),
        })
        .unwrap();
    assert!(
        db.bind_generation_representation(&scope, foreign.id, &key)
            .is_err()
    );
    assert!(
        db.generation_representation(&ScopeSet::new([].into()), generation.id)
            .unwrap()
            .is_none()
    );
}

fn spec(id: &str, profile_digest: Digest) -> RepresentationSetSpec {
    RepresentationSetSpec {
        collection_id: "collection".into(),
        chunk_set_id: "set".into(),
        id: id.into(),
        profile_digest,
        layout: RepresentationLayout {
            version: "dense-sparse/1".into(),
            embedding_profile: "dense".into(),
            sparse_profile: "sparse".into(),
        },
    }
}

fn shard(
    graph: &DeliveryGraph,
    graph_digest: &Digest,
    set: &str,
    members: usize,
) -> RepresentationShard {
    let mut entries = Vec::new();
    for view in &graph.retrieval_views {
        for membership in &view.memberships {
            if entries.len() == members {
                break;
            }
            entries.push(RepresentationMember {
                ordinal: entries.len() as u64,
                retrieval_view_id: view.retrieval_view_id.clone(),
                chunk_id: view.chunk_id.clone(),
                unit_id: membership.unit_id.clone(),
                input_digest: view.prepared_input_digest.clone(),
                mapping_digest: graph.descriptor.mapping_digest.clone(),
            });
        }
    }
    RepresentationShard {
        schema_version: "maestro-representation-shard/1".into(),
        representation_set_id: set.into(),
        revision_id: graph.descriptor.revision_id.clone(),
        graph_digest: graph_digest.clone(),
        members: entries,
    }
}

#[test]
fn representation_shard_read_checks_record_against_artifact() {
    use crate::store::Error as StoreError;
    let scratch = Scratch::new();
    let db = scratch.open();
    let graph = graph();
    let graph_digest = put(&db, &graph);
    db.record_revision_graph(&scoped(), "set", &graph_digest)
        .unwrap();
    let scope = ScopeSet::default_workspace();
    let key = RepresentationKey {
        collection_id: "collection",
        chunk_set_id: "set",
        id: "corrupt",
    };
    db.begin_representation_set(
        &scope,
        &spec("corrupt", graph.descriptor.profile_digest.clone()),
    )
    .unwrap();
    let shard = shard(&graph, &graph_digest, "different-set", 3);
    let digest = db
        .put(&shard.to_bytes().unwrap(), "application/json")
        .unwrap();
    db.write::<_, StoreError>(|tx| {
        tx.execute(
            "INSERT INTO representation_revisions
             VALUES ('collection','set','corrupt','revision',?1)",
            [digest.as_str()],
        )?;
        Ok(())
    })
    .unwrap();
    assert!(db.representation_shard(&scope, &key, "revision").is_err());
}

#[test]
fn representation_failed_state_cannot_complete() {
    let scratch = Scratch::new();
    let db = scratch.open();
    let graph = graph();
    let graph_digest = put(&db, &graph);
    db.record_revision_graph(&scoped(), "set", &graph_digest)
        .unwrap();
    let scope = ScopeSet::default_workspace();
    let key = RepresentationKey {
        collection_id: "collection",
        chunk_set_id: "set",
        id: "fail-then-complete",
    };
    db.begin_representation_set(
        &scope,
        &spec(
            "fail-then-complete",
            graph.descriptor.profile_digest.clone(),
        ),
    )
    .unwrap();
    let shard = shard(&graph, &graph_digest, "fail-then-complete", 3);
    let digest = db
        .put(&shard.to_bytes().unwrap(), "application/json")
        .unwrap();
    db.record_representation_revision(&scope, &key, &digest)
        .unwrap();
    db.complete_chunk_set("set", &graph_digest).unwrap();
    db.fail_representation_set(&scope, &key).unwrap();
    assert!(db.complete_representation_set(&scope, &key).is_err());
}

#[test]
fn representation_completion_refuses_a_missing_revision_shard() {
    let scratch = Scratch::new();
    let db = scratch.open();
    let graph = graph();
    let graph_digest = put(&db, &graph);
    db.record_revision_graph(&scoped(), "set", &graph_digest)
        .unwrap();
    hidden(&db);
    let mut hidden_graph = graph.clone();
    hidden_graph.descriptor.revision_id = "hidden-revision".into();
    hidden_graph.descriptor.document_id = "hidden-document".into();
    hidden_graph.retrieval_views[0].chunk_id = "hidden-chunk".into();
    db.write::<_, StoreError>(|tx| {
        tx.execute(
            "INSERT INTO chunks VALUES ('set','hidden-chunk','hidden-revision',NULL,?1,12,0,37)",
            [hidden_graph.retrieval_views[0]
                .prepared_input_digest
                .as_str()],
        )?;
        Ok(())
    })
    .unwrap();
    let hidden_digest = put(&db, &hidden_graph);
    db.record_revision_graph(&ScopeSet::default_workspace(), "set", &hidden_digest)
        .unwrap();
    let scope = ScopeSet::default_workspace();
    let key = RepresentationKey {
        collection_id: "collection",
        chunk_set_id: "set",
        id: "partial",
    };
    db.begin_representation_set(
        &scope,
        &spec("partial", graph.descriptor.profile_digest.clone()),
    )
    .unwrap();
    let base_shard = shard(&graph, &graph_digest, "partial", 3);
    let shard_digest = db
        .put(&base_shard.to_bytes().unwrap(), "application/json")
        .unwrap();
    db.record_representation_revision(&scope, &key, &shard_digest)
        .unwrap();
    db.complete_chunk_set("set", &graph_digest).unwrap();
    assert!(db.complete_representation_set(&scope, &key).is_err());
}
