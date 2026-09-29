//! Record/read atomicity and hidden-ancestry refusal.
use super::support::{Scratch, count, graph, hidden, put, scoped};
use crate::{
    artifact::Digest,
    scope::ScopeSet,
    unit_graph::{Error as GraphError, GraphKey, MappingLedger},
};

#[test]
fn graph_records_round_trip_exact_disjoint_seed_contributions() {
    let scratch = Scratch::new();
    let db = scratch.open();
    let graph = graph();
    let digest = put(&db, &graph);
    db.record_revision_graph(&scoped(), "set", &digest).unwrap();
    db.record_revision_graph(&scoped(), "set", &digest).unwrap();
    let key = GraphKey {
        collection_id: "collection",
        chunk_set_id: "set",
        revision_id: "revision",
    };
    let read = db.unit_graph(&scoped(), &key).unwrap().unwrap();
    assert_eq!(read, graph);
    let member = &read.retrieval_views[0].memberships[0];
    assert_eq!(member.primary_part_ids, ["part-3", "part-5"]);
    let context = read.required_context("rows").unwrap();
    assert_eq!(context, ["part-1", "part-2"]);
    assert_eq!(db.artifact(&digest).unwrap().unwrap().pins, 1);
    assert_eq!(
        db.artifact(&graph.descriptor.mapping_digest)
            .unwrap()
            .unwrap()
            .pins,
        1
    );
    assert_eq!(count(&db), 1);
    assert!(
        db.unit_graph(&ScopeSet::new([].into()), &key)
            .unwrap()
            .is_none()
    );
}

#[test]
fn graph_records_refuse_invalid_inputs_without_rows_or_pins() {
    let scratch = Scratch::new();
    let db = scratch.open();
    hidden(&db);
    let original = graph();
    for case in 0..6 {
        let mut changed = original.clone();
        match case {
            0 => changed.descriptor.collection_id = "foreign".into(),
            1 => changed.descriptor.revision_id = "unknown".into(),
            2 => changed.retrieval_views[0].chunk_id = "unknown".into(),
            3 => changed.descriptor.original_markdown_digest = Digest::of(b"wrong"),
            4 => changed.descriptor.mapping_digest = Digest::of(b"missing"),
            _ => {
                changed.descriptor.revision_id = "hidden-revision".into();
                changed.descriptor.document_id = "hidden-document".into();
            }
        }
        let digest = put(&db, &changed);
        assert!(
            db.record_revision_graph(&scoped(), "set", &digest).is_err(),
            "case {case}"
        );
        assert_eq!(count(&db), 0);
        assert_eq!(db.artifact(&digest).unwrap().unwrap().pins, 0);
    }
}

#[test]
fn graph_records_both_rank_arms_keep_the_same_delivery_ancestry() {
    use crate::unit_graph::RankPolicy;
    for policy in [RankPolicy::CompleteIdeas, RankPolicy::V2Unit] {
        let scratch = Scratch::new();
        let db = scratch.open();
        let mut graph = graph();
        graph.retrieval_views[0].rank_policy = policy;
        let digest = put(&db, &graph);
        db.record_revision_graph(&scoped(), "set", &digest).unwrap();
        let read = db
            .unit_graph(
                &scoped(),
                &GraphKey {
                    collection_id: "collection",
                    chunk_set_id: "set",
                    revision_id: "revision",
                },
            )
            .unwrap()
            .unwrap();
        let mut parent = read
            .units
            .iter()
            .find(|unit| unit.unit_id == "rows")
            .unwrap()
            .parent
            .as_deref();
        let mut chain = Vec::new();
        while let Some(id) = parent {
            chain.push(id);
            parent = read
                .groups
                .iter()
                .find(|group| group.group_id == id)
                .unwrap()
                .parent
                .as_deref();
        }
        assert_eq!(chain, ["row-group", "table", "section", "page"]);
        assert_eq!(read.retrieval_views[0].rank_policy, policy);
    }
}

#[test]
fn graph_records_missing_artifact_rolls_back_profile_and_graph() {
    use crate::store::Error;
    let scratch = Scratch::new();
    let db = scratch.open();
    let graph = graph();
    let digest = put(&db, &graph);
    // Keep CAS bytes readable but remove the unpinned registry entry: failure
    // must roll back the profile insert, not merely reject before the write.
    db.write::<_, Error>(|tx| {
        tx.execute(
            "DELETE FROM artifacts WHERE digest=?1",
            [graph.descriptor.mapping_digest.as_str()],
        )?;
        Ok(())
    })
    .unwrap();
    assert!(db.record_revision_graph(&scoped(), "set", &digest).is_err());
    assert_eq!(count(&db), 0);
    assert!(
        !db.reader()
            .unwrap()
            .prepare("SELECT 1 FROM chunk_set_profiles")
            .unwrap()
            .exists([])
            .unwrap()
    );
    assert_eq!(db.artifact(&digest).unwrap().unwrap().pins, 0);
}

#[test]
fn graph_records_hidden_family_member_is_not_exposed() {
    use crate::store::Error;
    let scratch = Scratch::new();
    let db = scratch.open();
    hidden(&db);
    let graph = graph();
    db.record_revision_graph(&scoped(), "set", &put(&db, &graph))
        .unwrap();
    let mut hidden = graph.clone();
    hidden.descriptor.revision_id = "hidden-revision".into();
    hidden.descriptor.document_id = "hidden-document".into();
    hidden.retrieval_views[0].chunk_id = "hidden-chunk".into();
    db.write::<_, Error>(|tx| {
        tx.execute(
            "INSERT INTO chunks VALUES ('set','hidden-chunk','hidden-revision',NULL,?1,12,0,37)",
            [hidden.retrieval_views[0].prepared_input_digest.as_str()],
        )?;
        Ok(())
    })
    .unwrap();
    let hidden_digest = put(&db, &hidden);
    assert!(matches!(
        db.record_revision_graph(&scoped(), "set", &hidden_digest),
        Err(GraphError::NotFound)
    ));
    db.record_revision_graph(&ScopeSet::default_workspace(), "set", &hidden_digest)
        .unwrap();
    let key = GraphKey {
        collection_id: "collection",
        chunk_set_id: "set",
        revision_id: "hidden-revision",
    };
    assert!(db.unit_graph(&scoped(), &key).unwrap().is_none());
    assert_eq!(
        db.unit_graph(&ScopeSet::default_workspace(), &key)
            .unwrap()
            .unwrap()
            .groups[0]
            .family,
        graph.groups[0].family
    );
    assert_eq!(count(&db), 2);
}

#[test]
fn graph_records_freeze_revision_chunk_membership_while_set_still_builds() {
    use crate::store::Error;
    let scratch = Scratch::new();
    let db = scratch.open();
    let graph = graph();
    db.record_revision_graph(&scoped(), "set", &put(&db, &graph))
        .unwrap();
    let added = db.write::<_, Error>(|tx| {
        tx.execute(
            "INSERT INTO chunks VALUES ('set','late-chunk','revision',NULL,?1,12,0,37)",
            [graph.retrieval_views[0].prepared_input_digest.as_str()],
        )?;
        Ok(())
    });
    assert!(
        added.is_err(),
        "a sealed graph cannot lose exact chunk membership"
    );
}

#[test]
fn graph_record_refuses_an_existing_but_mismatched_mapping_artifact() {
    use super::support::graph;
    let scratch = Scratch::new();
    let db = scratch.open();
    let mut graph = graph();
    let mut ledger = MappingLedger::from_bytes(
        include_bytes!("../../../tests/fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    ledger.contributions[0].mapping.unit_id = "wrong-canonical-unit".into();
    let mapping_digest = db
        .put(&ledger.to_bytes().unwrap(), "application/json")
        .unwrap();
    graph.descriptor.mapping_digest = mapping_digest;
    let graph_digest = db
        .put(&graph.to_bytes().unwrap(), "application/json")
        .unwrap();
    assert!(
        db.record_revision_graph(&scoped(), "set", &graph_digest)
            .is_err()
    );
    assert_eq!(count(&db), 0);
}

#[test]
fn graph_read_refuses_record_metadata_that_differs_from_artifact() {
    use crate::store::Error as StoreError;
    let scratch = Scratch::new();
    let db = scratch.open();
    let base = graph();
    db.record_revision_graph(&scoped(), "set", &put(&db, &base))
        .unwrap();
    hidden(&db);
    let mut hidden_graph = base.clone();
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
    let graph_digest = put(&db, &hidden_graph);
    let wrong_mapping = db.put(b"different mapping", "application/json").unwrap();
    db.write::<_, StoreError>(|tx| {
        tx.execute(
            "INSERT INTO revision_unit_graphs
             VALUES ('collection','set','hidden-revision','hidden-document',
                    ?1,?2,'maestro-unit-graph/1',?3)",
            [
                graph_digest.as_str(),
                wrong_mapping.as_str(),
                hidden_graph.descriptor.original_markdown_digest.as_str(),
            ],
        )?;
        Ok(())
    })
    .unwrap();
    let error = db
        .unit_graph(
            &ScopeSet::default_workspace(),
            &GraphKey {
                collection_id: "collection",
                chunk_set_id: "set",
                revision_id: "hidden-revision",
            },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        GraphError::Invalid("graph record differs from artifact")
    ));
}
