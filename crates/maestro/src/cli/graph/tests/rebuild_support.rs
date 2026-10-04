//! Real authority/artifact setup for optional G35 rebuild composition.
#![cfg(all(feature = "engine", not(windows)))]
use super::runner_tests::support::{Fixture, fixture};
use crate::cli::graph::rebuild_work::Selection;
use maestro_kernel::{
    artifact::Digest,
    chunk_set::{Chunk, NewChunkSet},
    evidence::Span,
    facts::{
        Batch, Budget, BuildPlan, Claim, EXACT_RESOLVER_VERSION, EntityKind, EntityName, Literal,
        LiteralKind, Object, Predicate, Provenance, ResolutionInput, Support, Validity,
    },
    generation::NewGeneration,
    job::{JobState, Lease, LeaseTiming, NewJob},
    scope::{LOCAL, collection_path},
};
use maestro_kernel::{document::Revision, store::Database};
use maestro_knowledge::graph::projection::ProjectionScope;
use maestro_knowledge::graph::verify::Source;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, time::SystemTime};

pub(in crate::cli::graph) fn authority() -> (Fixture, Selection) {
    let mut fixture = fixture(&["mode defaults to fast.\n"]);
    fixture.kernel.config_dir = fixture.root.clone();
    fs::write(
        fixture.root.join("config.toml"),
        "[access]\nread = ['workspace/default']\n",
    )
    .unwrap();
    fixture.kernel.refresh_scopes().unwrap();
    let db = &fixture.kernel.database;
    let scopes = &fixture.kernel.scopes;
    let revision = &fixture.revisions[0];
    let claim = claim(db, revision);
    let plan = BuildPlan {
        collection_id: "graph-test".into(),
        provenance: claim.provenance.clone(),
        sources: vec![revision.id.clone()],
        budget: Budget {
            max_claims: 1,
            max_rejections: 0,
        },
    };
    let timing = LeaseTiming {
        now: SystemTime::now(),
        term: super::super::super::lease::TIMING.term,
    };
    let mut build = lease(
        db,
        "knowledge.graph.build",
        &json!({"fixture":"descriptor"}),
        timing,
    );
    db.begin_graph_build(scopes, &build, &plan).unwrap();
    db.record_graph_batch(
        scopes,
        &mut build,
        timing,
        &Batch {
            ordinal: 0,
            claims: vec![claim],
            rejections: vec![],
        },
    )
    .unwrap();
    let set = db
        .finish_graph_build(scopes, &mut build, timing)
        .unwrap()
        .id;
    db.complete_job(&build, JobState::Succeeded, &json!({}))
        .unwrap();
    chunks(db, revision);
    let generation = db
        .create_generation(&NewGeneration {
            collection_id: "graph-test".into(),
            chunk_set_id: "descriptors".into(),
            embedding_profile: "test".into(),
            sparse_profile: "test".into(),
        })
        .unwrap()
        .id;
    let attach = lease(
        db,
        "knowledge.graph.attach",
        &json!({"build":build.job.to_string(), "generation":generation}),
        timing,
    );
    db.attach_claim_set(scopes, generation, build.job, &attach)
        .unwrap();
    db.complete_job(&attach, JobState::Succeeded, &json!({}))
        .unwrap();
    db.verify_generation(generation, 1).unwrap();
    let resolution = db
        .record_resolution(
            scopes,
            LOCAL,
            &ResolutionInput {
                resolver_version: EXACT_RESOLVER_VERSION.into(),
                sets: vec![set.clone()],
                previous: None,
                decisions: vec![],
            },
            &|_| Ok(()),
        )
        .unwrap()
        .id;
    let selection = Selection {
        scope: ProjectionScope {
            collection_id: "graph-test".into(),
            generation_id: generation,
        },
        claim_set: set,
        resolution,
    };
    (fixture, selection)
}

fn claim(db: &Database, revision: &Revision) -> Claim {
    let source = Source::read(db, revision).unwrap();
    let span = Span {
        start: 0,
        end: source.markdown().len() - 1,
    };
    Claim {
        subject: EntityName {
            kind: EntityKind::Parameter,
            name: "mode".into(),
        },
        predicate: Predicate::DefaultsTo,
        object: Object::Literal(Literal {
            kind: LiteralKind::Text,
            lexeme: "fast".into(),
        }),
        conditions: BTreeMap::new(),
        version: Validity::Unknown,
        world: Validity::Unknown,
        provenance: Provenance {
            extractor: "synthetic".into(),
            profile: Digest::of(b"profile"),
        },
        supports: vec![Support {
            revision_id: revision.id.clone(),
            block_id: source.canonical().blocks[0].block_id.clone(),
            span,
            quote_digest: Digest::of(&source.markdown().as_bytes()[..span.end]),
        }],
    }
}

fn chunks(db: &Database, revision: &Revision) {
    let span = Span { start: 0, end: 22 };
    db.begin_chunk_set(&NewChunkSet {
        id: "descriptors",
        collection_id: "graph-test",
        chunk_profile: "test",
        counter_contract_id: "test",
    })
    .unwrap();
    db.record_chunks(
        "descriptors",
        &revision.id,
        &[Chunk {
            id: "chunk".into(),
            revision_id: revision.id.clone(),
            section_id: None,
            digest: revision.original_digest.clone(),
            token_count: 4,
            span,
        }],
    )
    .unwrap();
    db.complete_chunk_set("descriptors", &db.put(b"{}", "application/json").unwrap())
        .unwrap();
}

fn lease(db: &Database, kind: &str, inputs: &Value, timing: LeaseTiming) -> Lease {
    let scope = collection_path("graph-test").parse().unwrap();
    let job = db
        .submit_job(
            &NewJob {
                kind,
                inputs,
                scope: &scope,
                resource: None,
            },
            timing.now,
        )
        .unwrap();
    db.take_job(job.id, "test", timing.now, timing.term)
        .unwrap()
}
