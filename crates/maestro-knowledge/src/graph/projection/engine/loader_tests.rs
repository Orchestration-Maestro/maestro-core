//! Real native loader recovery and immutable-record refusals.
#![cfg(unix)]
use super::public_fixture::{Fixture, now};
use crate::graph::build::inputs;
use crate::graph::projection::{
    BuildVerification, ProjectionSnapshot, TypedEdgeProjection,
    checkpoint::{Journal, Manifest, prefix},
};
use maestro_kernel::{
    facts::{
        Batch, Budget, BuildPlan, ClaimSetRecord, EXACT_RESOLVER_VERSION, EntityKind, Literal,
        LiteralKind, Object, Predicate, ResolutionInput,
    },
    generation::NewGeneration,
    job::{Lease, LeaseTiming, NewJob},
    scope::collection_path,
};
use serde_json::json;
use std::{collections::BTreeSet, fs, path::PathBuf, time::Duration};

/// Freeze a real attached set with enough rows to cross the loader's batch boundary.
pub(super) fn fixture(edge_count: usize) -> Fixture {
    let mut fixture = Fixture::new();
    let database = &fixture.authority.database;
    let scopes = &fixture.authority.scopes;
    let timing = LeaseTiming {
        now: now(0),
        term: Duration::from_secs(60),
    };
    let (set, builder) = freeze(&fixture, edge_count, timing);
    let scope = collection_path(&fixture.build.scope.collection_id)
        .parse()
        .unwrap();
    let generation = database
        .create_generation(&NewGeneration {
            collection_id: fixture.build.scope.collection_id.clone(),
            chunk_set_id: "chunks".into(),
            embedding_profile: "synthetic".into(),
            sparse_profile: "synthetic".into(),
        })
        .unwrap()
        .id;
    let body = json!({"build": builder.job.to_string(), "generation":generation});
    let job = database
        .submit_job(
            &NewJob {
                kind: "knowledge.graph.attach",
                inputs: &body,
                scope: &scope,
                resource: None,
            },
            timing.now,
        )
        .unwrap();
    let attach = database
        .take_job(job.id, "loader", timing.now, timing.term)
        .unwrap();
    database
        .attach_claim_set(scopes, generation, builder.job, &attach)
        .unwrap();
    database.verify_generation(generation, 1).unwrap();
    let resolution = database
        .record_resolution(
            scopes,
            "builder",
            &ResolutionInput {
                resolver_version: EXACT_RESOLVER_VERSION.into(),
                sets: vec![set.id.clone()],
                previous: None,
                decisions: vec![],
            },
            &|_| Ok(()),
        )
        .unwrap();
    let body = json!({"generation":generation});
    let job = database
        .submit_job(
            &NewJob {
                kind: "knowledge.graph.project",
                inputs: &body,
                scope: &scope,
                resource: None,
            },
            timing.now,
        )
        .unwrap();
    fixture.build.lease = database
        .take_job(job.id, "loader", timing.now, timing.term)
        .unwrap();
    fixture.build.scope.generation_id = generation;
    fixture.build.claim_set_id = set.id;
    fixture.build.resolution_id = resolution.id;
    fixture
}

/// Source-backed claims and the build lease used by the normal attachment API.
fn freeze(fixture: &Fixture, edge_count: usize, timing: LeaseTiming) -> (ClaimSetRecord, Lease) {
    let database = &fixture.authority.database;
    let scopes = &fixture.authority.scopes;
    let original = database
        .claim_set(scopes, &fixture.build.claim_set_id)
        .unwrap()
        .unwrap();
    let source = original.claims[0].claim.clone();
    let mut claims: Vec<_> = (0..edge_count)
        .map(|index| {
            let mut claim = source.clone();
            claim
                .conditions
                .insert("loader-row".into(), index.to_string());
            claim
        })
        .collect();
    if edge_count > 0 {
        let mut literal = source.clone();
        literal.subject.kind = EntityKind::Parameter;
        literal.predicate = Predicate::DefaultsTo;
        literal.object = Object::Literal(Literal {
            kind: LiteralKind::Text,
            lexeme: "é雪 '\"\n$bound".into(),
        });
        claims.push(literal);
    }
    let plan = BuildPlan {
        collection_id: fixture.build.scope.collection_id.clone(),
        provenance: source.provenance,
        sources: source
            .supports
            .iter()
            .map(|support| support.revision_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        budget: Budget {
            max_claims: claims.len().max(1),
            max_rejections: 0,
        },
    };
    let scope = collection_path(&plan.collection_id).parse().unwrap();
    let body = inputs(&plan, None);
    let job = database
        .submit_job(
            &NewJob {
                kind: "knowledge.graph.build",
                inputs: &body,
                scope: &scope,
                resource: None,
            },
            timing.now,
        )
        .unwrap();
    let mut builder = database
        .take_job(job.id, "loader", timing.now, timing.term)
        .unwrap();
    database.begin_graph_build(scopes, &builder, &plan).unwrap();
    database
        .record_graph_batch(
            scopes,
            &mut builder,
            timing,
            &Batch {
                ordinal: 0,
                claims,
                rejections: vec![],
            },
        )
        .unwrap();
    let set = database
        .finish_graph_build(scopes, &mut builder, timing)
        .unwrap();
    (set, builder)
}

/// The public read is the sole source of rows; no test-made pins or row IDs.
pub(super) fn snapshot(fixture: &Fixture) -> ProjectionSnapshot {
    ProjectionSnapshot::read(
        &fixture.authority.database,
        &fixture.authority.scopes,
        "builder",
        (&fixture.build.claim_set_id, &fixture.build.resolution_id),
        &fixture.build.scope,
    )
    .unwrap()
}

pub(super) fn staging(fixture: &Fixture) -> PathBuf {
    fixture
        .native
        .path
        .join(format!(".build-{}", fixture.build.lease.job))
}

#[test]
fn loader_repeated_clean_and_resumed_both_families_agree() {
    let fixture = fixture(65);
    let snapshot = snapshot(&fixture);
    let factory = fixture.factory();
    let clock = || now(0);
    let mut producer = factory
        .producer(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    producer.load(&snapshot).unwrap();
    let path = staging(&fixture).join("loader");
    let manifest = fs::read(path.join("manifest.json")).unwrap();
    let first = fs::read(path.join("0000000001.json")).unwrap();
    producer.load(&snapshot).unwrap();
    assert_eq!(fs::read(path.join("manifest.json")).unwrap(), manifest);
    assert_eq!(fs::read(path.join("0000000001.json")).unwrap(), first);
    let expected = BuildVerification::expected(&snapshot.edges, &snapshot.facts).unwrap();
    assert_eq!(producer.verify().unwrap(), expected);
    drop(producer);
    let mut resumed = factory
        .resume(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    resumed.load(&snapshot).unwrap();
    assert_eq!(resumed.verify().unwrap(), expected);
    let published = resumed.publish(&expected).unwrap();
    assert_eq!(published.receipt.identity.entity_fact_count, 1);
    assert_eq!(published.receipt.identity.knowledge_edge_count, 65);
    assert!(!path.exists());
    let reader = factory
        .reader(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
        )
        .unwrap();
    assert_eq!(
        reader
            .entity_facts(
                &fixture.authority.scopes,
                &fixture.build.scope,
                &snapshot.facts[0].subject
            )
            .unwrap(),
        snapshot.facts
    );
    assert!(
        factory
            .resume(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.clone(),
                &clock
            )
            .is_err()
    );
    // A failed recovery cannot invalidate an existing immutable reader pin.
    assert_eq!(
        reader
            .entity_facts(
                &fixture.authority.scopes,
                &fixture.build.scope,
                &snapshot.facts[0].subject
            )
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn loader_uncertain_committed_batch_is_rechecked_not_rewritten_after_takeover() {
    let mut fixture = fixture(65);
    let snapshot = snapshot(&fixture);
    let clock = || now(0);
    let mut producer = fixture
        .factory()
        .producer(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    // Exact crash window: immutable manifest, native batch committed, no checkpoint yet.
    Journal::create(
        &staging(&fixture),
        Manifest::expected(&fixture.build, &snapshot).unwrap(),
    )
    .unwrap();
    producer.write_batch(&snapshot.edges[..64], &[]).unwrap();
    assert_eq!(producer.verify().unwrap(), prefix(&snapshot, 1).unwrap());
    drop(producer);
    let stale = fixture.build.clone();
    fixture.build.lease = fixture
        .authority
        .database
        .take_job(
            stale.lease.job,
            "successor",
            now(60),
            Duration::from_secs(60),
        )
        .unwrap();
    let clock = || now(60);
    assert!(
        fixture
            .factory()
            .resume(
                &fixture.authority.database,
                &fixture.authority.scopes,
                stale,
                &clock
            )
            .is_err()
    );
    let mut resumed = fixture
        .factory()
        .resume(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.clone(),
            &clock,
        )
        .unwrap();
    resumed.load(&snapshot).unwrap();
    let expected = prefix(&snapshot, 2).unwrap();
    assert_eq!(resumed.verify().unwrap(), expected);
    resumed.publish(&expected).unwrap();
}
