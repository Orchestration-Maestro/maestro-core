//! A prepared chunk set and generation for evidence-path tests.

use crate::search::evidence::{CounterMode, EvidenceSettings};
use crate::{
    prepare::{
        ChunkProfile, Preparation, prepare_observed,
        tests::scratch::{COLLECTION, Scratch, decide_all, router_tokenizer},
    },
    query::understand,
    search::{Candidate, EvidenceInput, Hit, Ranked, Route, RouteList, SearchObservations, fuse},
};
use maestro_kernel::{
    document::Outcome,
    evidence::{RequestBudget, RouteStatus},
    generation::{Generation, NewGeneration},
    retrieval::ReadControl,
    scope::ScopeSet,
    store::Database,
};
use std::{
    collections::BTreeMap,
    ops::ControlFlow,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use tokio::time::Instant as TokioInstant;

/// One synthetic, authorized corpus prepared and pinned to a generation.
pub(super) struct Fixture {
    /// The kernel used to prepare and read the evidence.
    pub(super) database: Database,
    /// The read grants captured during import.
    pub(super) scopes: ScopeSet,
    /// The generation pinned to the chunk set.
    pub(super) generation: Generation,
    /// The source directory lives as long as this test fixture. Last, as
    /// fields drop in order: Windows refuses to remove a database still open.
    pub(super) scratch: Scratch,
}

/// Imports, accepts, prepares and publishes a synthetic corpus.
pub(super) fn fixture(documents: &[(&str, &str)]) -> Fixture {
    fixture_under(documents, ChunkProfile::Structural)
}

/// [`fixture`], its chunk set cut under `profile`.
pub(super) fn fixture_under(documents: &[(&str, &str)], profile: ChunkProfile) -> Fixture {
    let scratch = Scratch::new();
    scratch.corpus(documents);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    decide_all(&database, &scopes, Outcome::Accepted);
    let counter = router_tokenizer();
    let preparation = Preparation {
        collection: COLLECTION,
        profile,
    };
    let mut unobserved = |_: &_| ControlFlow::Continue(());
    let report =
        prepare_observed(&database, &scopes, preparation, &counter, &mut unobserved).unwrap();
    let building = database
        .create_generation(&NewGeneration {
            collection_id: COLLECTION.to_owned(),
            chunk_set_id: report.chunk_set.clone(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap();
    database
        .verify_generation(building.id, report.chunks)
        .unwrap();
    database.publish_generation(building.id).unwrap();
    let generation = database.generation(&scopes, building.id).unwrap().unwrap();
    Fixture {
        database,
        scopes,
        generation,
        scratch,
    }
}

/// As [`evidence_input`], configured for an exact token counter.
pub(super) fn exact_evidence_input(fixture: &Fixture, query: &str) -> EvidenceInput {
    let mut input = evidence_input(fixture, query);
    input.evidence.evidence_counter = CounterMode::Exact;
    input
}

/// Builds a bounded search handoff from every chunk in the fixture generation.
pub(super) fn evidence_input(fixture: &Fixture, query: &str) -> EvidenceInput {
    let chunks = fixture
        .database
        .chunks(&fixture.scopes, &fixture.generation.chunk_set_id)
        .unwrap();
    let hits = chunks
        .iter()
        .map(|chunk| Hit {
            chunk_id: chunk.id.clone(),
            score: 1.0,
        })
        .collect();
    let fused = fuse(
        &[RouteList {
            route: Route::Lexical,
            hits,
        }],
        120,
    );
    let ranked = fused
        .into_iter()
        .map(|fused| Ranked {
            candidate: Candidate {
                header: None,
                fused,
                text: "untrusted candidate text".to_owned(),
            },
            score: Some(0.8),
        })
        .collect();
    EvidenceInput {
        evidence: EvidenceSettings::default(),
        generation: fixture.generation.clone(),
        query: query.to_owned(),
        understood: understand(query),
        version: None,
        principal: "tester".to_owned(),
        scopes: Arc::new(fixture.scopes.clone()),
        ranked,
        observations: SearchObservations::default(),
        routes: BTreeMap::from([
            ("lexical".to_owned(), RouteStatus::Ok),
            ("rerank".to_owned(), RouteStatus::Ok),
        ]),
        inventory: None,
        budget: RequestBudget::default(),
        deadline: TokioInstant::now() + Duration::from_secs(10),
        known_gaps: Vec::new(),
    }
}

/// A live read control long enough for deterministic local tests.
pub(in crate::search::evidence) fn control() -> ReadControl {
    ReadControl {
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: Arc::new(AtomicBool::new(false)),
    }
}
