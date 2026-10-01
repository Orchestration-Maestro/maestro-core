//! The reranker health gate admits only finite, separated public-pair scores.

use super::super::{
    RerankerEvaluation, check_reranker_health, evaluate_and_record_reranker_health,
    record_reranker_health,
};
use crate::prepare::tests::support::identity;
use maestro_kernel::{
    artifact::{Digest, Store},
    document::Collection,
    gateway::{
        ChatRequest, Error, FakeModels, ModelCard, ModelPort, Role, Room,
        card_v2::{Capability, Dimensions, EmbeddingFormat, QualificationMethod},
    },
    model::{Error as ModelError, EvaluationDisposition, EvaluationMode, NewModelSelection},
    scope::{Right, Scope, ScopeSet},
    store::{Database, Error as StoreError},
};
use maestro_test_scratch::scratch_directory;
use std::{
    collections::BTreeMap,
    fs,
    future::{Future, ready},
    path::PathBuf,
    sync::{Arc, Mutex},
};

type PairCalls = Arc<Mutex<Vec<(String, Vec<String>)>>>;

#[derive(Debug, Clone)]
struct ScoredPairs {
    scores: Arc<Mutex<Vec<Vec<f64>>>>,
    calls: PairCalls,
}

impl ScoredPairs {
    fn new(scores: [[f64; 2]; 2]) -> Self {
        Self {
            scores: Arc::new(Mutex::new(scores.into_iter().map(Vec::from).collect())),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_score_vectors(scores: [Vec<f64>; 2]) -> Self {
        Self {
            scores: Arc::new(Mutex::new(scores.into_iter().collect())),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ModelPort for ScoredPairs {
    async fn embed(
        &self,
        card: &ModelCard,
        room: Room,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, Error> {
        FakeModels.embed(card, room, inputs).await
    }

    fn rerank(
        &self,
        _card: &ModelCard,
        _room: Room,
        query: &str,
        documents: &[String],
    ) -> impl Future<Output = Result<Vec<f64>, Error>> {
        self.calls
            .lock()
            .unwrap()
            .push((query.to_owned(), documents.to_vec()));
        ready(Ok(self.scores.lock().unwrap().remove(0)))
    }

    async fn tokenize(&self, card: &ModelCard, room: Room, text: &str) -> Result<Vec<u32>, Error> {
        FakeModels.tokenize(card, room, text).await
    }

    async fn chat(
        &self,
        card: &ModelCard,
        room: Room,
        request: &ChatRequest,
    ) -> Result<String, Error> {
        FakeModels.chat(card, room, request).await
    }
}

struct Fixture {
    database: Option<Database>,
    scopes: ScopeSet,
    card: ModelCard,
    collection: &'static str,
    scratch: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let scratch = scratch_directory().unwrap();
        let database =
            Database::open(&scratch.join("kernel.sqlite3"), &scratch.join("artifacts")).unwrap();
        let collection = "health-test";
        database
            .record_collection(&Collection {
                id: collection.to_owned(),
                title: collection.to_owned(),
                visibility: "private".to_owned(),
                profiles: BTreeMap::new(),
            })
            .unwrap();
        let collection_scope: Scope = "workspace/default/collection/health-test".parse().unwrap();
        database
            .grant("health-test", &collection_scope, Right::Read, "test")
            .unwrap();
        let scopes = database.visible("health-test").unwrap();
        let qualification_digest = database
            .put(b"public reranker health qualification", "application/json")
            .unwrap();
        let mut identity = identity(qualification_digest);
        identity.role = Role::Reranker;
        identity.invocation.dimensions = Dimensions::NotApplicable;
        identity.formats.embedding = EmbeddingFormat::NotApplicable;
        identity.formats.document = Capability::NotApplicable;
        identity.formats.query = Capability::NotApplicable;
        identity.provenance.qualification_method = QualificationMethod::NativeRuntime;
        let card = ModelCard::record_v2(&Store::new(scratch.join("cards")), &identity).unwrap();
        database
            .record_model_card(
                &scopes,
                &maestro_kernel::model::NewModelCard {
                    collection_id: collection,
                    card: &card,
                },
            )
            .unwrap();
        Self {
            database: Some(database),
            scopes,
            card,
            collection,
            scratch,
        }
    }

    fn database(&self) -> &Database {
        self.database.as_ref().unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.database.take());
        fs::remove_dir_all(&self.scratch).unwrap();
    }
}

#[tokio::test]
async fn public_pair_health_receipt_is_bound_to_the_exact_card() {
    let fixture = Fixture::new();
    let port = ScoredPairs::new([[0.7, 0.2], [0.4, 0.1]]);
    let health = check_reranker_health(&port, &fixture.card).await;

    assert_eq!(health.disposition(), EvaluationDisposition::Eligible);
    assert_eq!(health.card_digest(), fixture.card.digest());
    let receipt = serde_json::to_value(health.receipt()).unwrap();
    assert_eq!(receipt["card_digest"], fixture.card.digest().as_str());
    assert_eq!(receipt["qualification_method"], "native_runtime");
    assert_eq!(receipt["pairs"][0]["spread"], 0.7 - 0.2);
    assert_eq!(receipt["pairs"][1]["spread"], 0.4 - 0.1);
    assert_eq!(
        port.calls.lock().unwrap().as_slice(),
        [
            (
                "What gas do plants absorb during photosynthesis?".to_owned(),
                vec![
                    "Plants absorb carbon dioxide during photosynthesis.".to_owned(),
                    "Plants use sunlight to power growth.".to_owned(),
                ],
            ),
            (
                "Which planet is known as the Red Planet?".to_owned(),
                vec![
                    "Mars is commonly known as the Red Planet.".to_owned(),
                    "Saturn is known for its prominent rings.".to_owned(),
                ],
            ),
        ]
    );
}

#[tokio::test]
async fn a_health_receipt_cannot_be_recorded_against_another_card() {
    let fixture = Fixture::new();
    let health =
        check_reranker_health(&ScoredPairs::new([[0.7, 0.2], [0.4, 0.1]]), &fixture.card).await;
    let mut identity = fixture.card.identity().unwrap().clone();
    identity.invocation.llama_cpp_build = "other-build".to_owned();
    let other =
        ModelCard::record_v2(&Store::new(fixture.scratch.join("other-cards")), &identity).unwrap();

    let error = record_reranker_health(
        fixture.database(),
        &fixture.scopes,
        &other,
        &health,
        &RerankerEvaluation {
            collection_id: fixture.collection,
            run_id: "mismatched-health-card",
            generation_id: None,
            manifest: b"public health manifest",
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("belongs to another model card"));
}

#[tokio::test]
async fn a_spread_exactly_at_the_minimum_is_eligible() {
    let fixture = Fixture::new();
    let health =
        check_reranker_health(&ScoredPairs::new([[0.1, 0.0], [0.2, 0.0]]), &fixture.card).await;

    assert_eq!(health.disposition(), EvaluationDisposition::Eligible);
    let receipt = serde_json::to_value(health.receipt()).unwrap();
    assert_eq!(receipt["pairs"][0]["spread"], 0.1);
}

#[tokio::test]
async fn missing_pair_scores_are_ineligible() {
    let fixture = Fixture::new();
    let port = ScoredPairs::with_score_vectors([Vec::new(), vec![0.5, 0.0]]);
    let health = check_reranker_health(&port, &fixture.card).await;

    assert_eq!(health.disposition(), EvaluationDisposition::Ineligible);
    assert!(health.failure().unwrap().contains("expected two scores"));
}

#[tokio::test]
async fn nonfinite_reversed_flat_and_below_spread_scores_are_ineligible() {
    let fixture = Fixture::new();
    for scores in [
        [[f64::NAN, 0.0], [0.5, 0.0]],
        [[0.5, 0.0], [f64::NAN, 0.0]],
        [[0.1, 0.2], [0.5, 0.0]],
        [[0.5, 0.0], [0.1, 0.2]],
        [[0.2, 0.2], [0.5, 0.0]],
        [[0.0, 0.0], [0.5, 0.0]],
        [[0.09, 0.0], [0.5, 0.0]],
        [[0.5, 0.0], [0.09, 0.0]],
    ] {
        let port = ScoredPairs::new(scores);
        let health = check_reranker_health(&port, &fixture.card).await;
        assert_eq!(health.disposition(), EvaluationDisposition::Ineligible);
        assert!(health.failure().is_some());
        if scores[0][0].is_nan() || scores[0][1].is_nan() {
            assert!(
                health
                    .failure()
                    .unwrap()
                    .contains("pair scores must be finite")
            );
        }
    }
}

#[tokio::test]
async fn ineligible_health_records_an_ineligible_real_evaluation_and_cannot_be_selected() {
    let fixture = Fixture::new();
    let port = ScoredPairs::new([[0.2, 0.2], [0.5, 0.0]]);
    let (health, evaluation) = evaluate_and_record_reranker_health(
        fixture.database(),
        &fixture.scopes,
        &fixture.card,
        &port,
        &RerankerEvaluation {
            collection_id: fixture.collection,
            run_id: "health-run",
            generation_id: None,
            manifest: b"public health manifest",
        },
    )
    .await
    .unwrap();
    assert_eq!(health.disposition(), EvaluationDisposition::Ineligible);

    assert_eq!(evaluation.mode, EvaluationMode::Real);
    assert_eq!(evaluation.disposition, EvaluationDisposition::Ineligible);
    let error = fixture
        .database()
        .record_model_selection(
            &fixture.scopes,
            &NewModelSelection {
                collection_id: fixture.collection,
                role: Role::Reranker,
                card_id: evaluation.card_id,
                evaluation_id: evaluation.id,
                selected_by: "owner",
                reason: "health failed",
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("eligible real evaluation"));
}

#[test]
fn missing_build_or_unpinned_qualification_artifact_cannot_become_eligible() {
    let fixture = Fixture::new();
    let mut identity = fixture.card.identity().unwrap().clone();
    identity.invocation.llama_cpp_build.clear();
    let buildless = ModelCard::record_v2(
        &Store::new(fixture.scratch.join("buildless-card")),
        &identity,
    );
    assert!(buildless.is_err());

    identity = fixture.card.identity().unwrap().clone();
    identity.formats.qualification_digest = Digest::of(b"not pinned in the registry");
    let unpinned = ModelCard::record_v2(
        &Store::new(fixture.scratch.join("unpinned-card")),
        &identity,
    )
    .unwrap();
    let error = fixture
        .database()
        .record_model_card(
            &fixture.scopes,
            &maestro_kernel::model::NewModelCard {
                collection_id: fixture.collection,
                card: &unpinned,
            },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        ModelError::Store(StoreError::UnknownArtifact(_))
    ));
}

#[tokio::test]
async fn the_health_gate_refuses_a_card_with_the_wrong_qualification_method() {
    let fixture = Fixture::new();
    let mut identity = fixture.card.identity().unwrap().clone();
    identity.provenance.qualification_method = QualificationMethod::NativeTokenizer;
    let other =
        ModelCard::record_v2(&Store::new(fixture.scratch.join("other-cards")), &identity).unwrap();
    let port = ScoredPairs::new([[0.7, 0.2], [0.4, 0.1]]);
    let health = check_reranker_health(&port, &other).await;

    assert_eq!(health.disposition(), EvaluationDisposition::Blocked);
    assert!(port.calls.lock().unwrap().is_empty());
}

#[test]
fn generic_registry_still_refuses_selection_without_a_real_evaluation() {
    let fixture = Fixture::new();
    let card_id = fixture
        .database()
        .model_cards(&fixture.scopes, fixture.collection, Role::Reranker)
        .unwrap()[0]
        .id;
    let synthetic = fixture
        .database()
        .record_model_evaluation(
            &fixture.scopes,
            &maestro_kernel::model::NewModelEvaluation {
                run_id: "synthetic-health",
                collection_id: fixture.collection,
                card_id,
                role: Role::Reranker,
                mode: EvaluationMode::Synthetic,
                generation_id: None,
                disposition: EvaluationDisposition::Eligible,
                manifest: b"manifest",
                report: b"report",
            },
        )
        .unwrap();
    let error = fixture
        .database()
        .record_model_selection(
            &fixture.scopes,
            &NewModelSelection {
                collection_id: fixture.collection,
                role: Role::Reranker,
                card_id,
                evaluation_id: synthetic.id,
                selected_by: "owner",
                reason: "not real",
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("eligible real evaluation"));
}
