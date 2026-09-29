//! The search's candidate stage end to end: exact texts, optional context,
//! the recorded reranker input, the rank policies and the assembled bundle.

use super::rerank::{FakePort, card};
use crate::{
    prepare::{
        prepare,
        tests::scratch::{COLLECTION, Scratch, decide_all, router_tokenizer},
    },
    query::understand,
    search::{
        CandidateContext, EvidenceInput, Hit, Ranked, Route, RouteList, SearchConfiguration,
        SearchObservations, SectionClassSet, SectionPrior, SourceClassSet, SourceClassTable,
        SourceClassifier, SourcePrior,
        admission::AdmittedSearch,
        deadline,
        evidence::{EvidenceCounter, EvidenceSettings, assemble_evidence},
        fuse,
        rank_stage::{self, Pool, Ranking},
        rerank::Reranker,
        top_rerank_score,
    },
};
use maestro_kernel::{
    chunk_set::Chunk,
    document::Outcome,
    evidence::{RequestBudget, RouteStatus},
    gateway::Role,
    generation::{Generation, NewGeneration},
    scope::ScopeSet,
    store::Database,
};
use std::{
    collections::{BTreeMap, HashMap},
    num::NonZeroUsize,
    sync::Arc,
};
use tokio::time::{Duration, Instant};

const RELEASE: &str = "# Release notes\n\nRun was renamed in this release.\n";
const GUIDE: &str = "# Running\n\nStart here.\n\n1. Select a task.\n2. Press Run.\n";

/// One published two-document corpus, its chunks in fused order: the
/// release note, then the guide's introduction and its steps, which the
/// chunker splits apart.
struct Corpus {
    database: Arc<Database>,
    scopes: ScopeSet,
    generation: Generation,
    chunks: Vec<Chunk>,
    /// Last, as fields drop in order: Windows refuses to remove a database
    /// still open.
    _scratch: Scratch,
}

impl Corpus {
    fn new() -> Self {
        let scratch = Scratch::new();
        scratch.corpus(&[("release.md", RELEASE), ("guide.md", GUIDE)]);
        let database = Arc::new(scratch.database());
        let scopes = scratch.import(&database);
        decide_all(&database, &scopes, Outcome::Accepted);
        let report = prepare(&database, &scopes, COLLECTION, &router_tokenizer()).unwrap();
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
        let mut chunks = database.chunks(&scopes, &generation.chunk_set_id).unwrap();
        chunks.sort_by_key(|chunk| {
            let text = Self::text(&database, chunk);
            ["renamed", "Start here.", "Press Run."]
                .iter()
                .position(|word| text.contains(word))
        });
        assert_eq!(chunks.len(), 3);
        Self {
            database,
            scopes,
            generation,
            chunks,
            _scratch: scratch,
        }
    }

    /// The prepared input the index holds for `chunk`.
    fn text(database: &Database, chunk: &Chunk) -> String {
        String::from_utf8(database.get(&chunk.digest).unwrap()).unwrap()
    }

    fn prepared(&self) -> Vec<String> {
        self.chunks
            .iter()
            .map(|chunk| Self::text(&self.database, chunk))
            .collect()
    }

    fn admitted(&self, query: &str, configuration: SearchConfiguration) -> AdmittedSearch {
        AdmittedSearch {
            understood: understand(query),
            structured_request: None,
            structured_error: None,
            version: None,
            version_documented: true,
            generation: self.generation.clone(),
            principal: "tester".to_owned(),
            scopes: Arc::new(self.scopes.clone()),
            cutoffs: deadline::from_budget(
                Instant::now(),
                RequestBudget {
                    deadline_ms: 10_000,
                    ..RequestBudget::default()
                },
                configuration.stage_window,
            ),
            configuration,
            source_classes: None,
        }
    }

    fn pool(&self) -> Pool {
        let hits = self
            .chunks
            .iter()
            .map(|chunk| Hit {
                chunk_id: chunk.id.clone(),
                score: 1.0,
            })
            .collect();
        Pool {
            fused: fuse(
                &[RouteList {
                    route: Route::Lexical,
                    hits,
                }],
                120,
            ),
            expected_revisions: self
                .chunks
                .iter()
                .map(|chunk| (chunk.id.clone(), vec![chunk.revision_id.clone()]))
                .collect::<HashMap<_, _>>(),
            rerank_extra: 0,
            known_scores: HashMap::new(),
        }
    }

    async fn rank(&self, port: &FakePort, admitted: &AdmittedSearch, query: &str) -> Ranking {
        let card = card(Role::Reranker, 8192);
        let reranker = Reranker { port, card: &card };
        rank_stage::rank(
            self.database.clone(),
            Some(&reranker),
            admitted,
            query,
            self.pool(),
        )
        .await
        .unwrap()
    }

    fn id(&self, index: usize) -> &str {
        &self.chunks[index].id
    }
}

fn bounded() -> SearchConfiguration {
    SearchConfiguration {
        candidate_context: CandidateContext::BoundedSection { max_bytes: 1500 },
        ..SearchConfiguration::default()
    }
}

fn prior(weight: f32) -> SectionPrior {
    let mut classes = SectionClassSet::default();
    for name in ["changelog", "release_notes", "conversion"] {
        assert!(classes.insert(name));
    }
    SectionPrior::Soft { weight, classes }
}

fn texts(ranking: &Ranking) -> Vec<&str> {
    ranking
        .ranked
        .iter()
        .map(|item| item.candidate.text.as_str())
        .collect()
}

fn order(ranking: &Ranking) -> Vec<&str> {
    ranking
        .ranked
        .iter()
        .map(|item| item.candidate.fused.chunk_id.as_str())
        .collect()
}

#[tokio::test]
async fn the_reranker_reads_chunk_text_by_default() {
    let corpus = Corpus::new();
    let port = FakePort::scores(vec![0.9, 0.5, 0.1]);
    let admitted = corpus.admitted("run a task", SearchConfiguration::default());
    let ranking = corpus.rank(&port, &admitted, "run a task").await;
    assert_eq!(port.calls.lock().unwrap()[0].documents, corpus.prepared());
    assert_eq!(texts(&ranking), corpus.prepared());
    assert_eq!(ranking.status, RouteStatus::Ok);
    assert!(ranking.fallbacks.is_empty());
    assert_eq!(ranking.context_gap(), None);
}

#[tokio::test]
async fn bounded_context_reaches_the_reranker_only_within_its_depth() {
    let corpus = Corpus::new();
    let port = FakePort::scores(vec![0.9, 0.5]);
    let configuration = SearchConfiguration {
        rerank_depth: NonZeroUsize::new(2).unwrap(),
        ..bounded()
    };
    let admitted = corpus.admitted("run a task", configuration);
    let ranking = corpus.rank(&port, &admitted, "run a task").await;
    let sent = port.calls.lock().unwrap()[0].documents.clone();
    assert_eq!(sent.len(), 2);
    assert_ne!(sent[0], corpus.prepared()[0]);
    assert!(sent[0].contains("Run was renamed in this release."));
    assert!(sent[1].contains("Start here.\n\n1. Select a task.\n2. Press Run."));
    assert_eq!(texts(&ranking)[..2], sent);
    assert_eq!(texts(&ranking)[2], corpus.prepared()[2]);
    assert!(ranking.fallbacks.is_empty());
}

#[tokio::test]
async fn a_disabled_rerank_expands_nothing() {
    let corpus = Corpus::new();
    let port = FakePort::scores(vec![0.9, 0.5, 0.1]);
    let configuration = SearchConfiguration {
        rerank_enabled: false,
        ..bounded()
    };
    let admitted = corpus.admitted("run a task", configuration);
    let ranking = corpus.rank(&port, &admitted, "run a task").await;
    assert!(port.calls.lock().unwrap().is_empty());
    assert_eq!(texts(&ranking), corpus.prepared());
    assert!(ranking.fallbacks.is_empty());
}

#[tokio::test]
async fn bounded_context_leaves_the_bundle_byte_identical() {
    let corpus = Corpus::new();
    let mut bundles = Vec::new();
    for configuration in [SearchConfiguration::default(), bounded()] {
        let port = FakePort::scores(vec![0.5, 0.5, 0.5]);
        let admitted = corpus.admitted("run a task", configuration);
        let ranking = corpus.rank(&port, &admitted, "run a task").await;
        let input = evidence_input(&corpus, admitted, ranking.ranked);
        let bundle = assemble_evidence(corpus.database.clone(), input, EvidenceCounter::Utf8Bytes)
            .await
            .unwrap();
        assert!(!bundle.passages.is_empty());
        bundles.push(serde_json::to_vec(&bundle.passages).unwrap());
    }
    assert_eq!(bundles[0], bundles[1]);
}

#[tokio::test]
async fn the_prior_demotes_a_release_note_for_a_generic_question_before_the_cap() {
    let corpus = Corpus::new();
    let (release, intro, steps) = (corpus.id(0), corpus.id(1), corpus.id(2));
    let configuration = SearchConfiguration {
        section_prior: prior(0.5),
        ..SearchConfiguration::default()
    };
    let port = FakePort::scores(vec![0.9, 0.5, 0.1]);
    let admitted = corpus.admitted("run a task", configuration);
    let demoted = corpus.rank(&port, &admitted, "run a task").await;
    assert_eq!(order(&demoted), [intro, release, steps]);
    assert_eq!(texts(&demoted)[0], corpus.prepared()[1]);

    let explicit = "show the release notes";
    let admitted = corpus.admitted(explicit, configuration);
    let exempt = corpus.rank(&port, &admitted, explicit).await;
    assert_eq!(order(&exempt), [release, intro, steps]);

    let capped = SearchConfiguration {
        rerank_demotion_cap: Some(0),
        ..configuration
    };
    let admitted = corpus.admitted("run a task", capped);
    let bounded_demotion = corpus.rank(&port, &admitted, "run a task").await;
    assert_eq!(order(&bounded_demotion), [release, intro, steps]);
}

#[tokio::test]
async fn enrichment_past_its_cutoff_keeps_chunk_text_and_reports_a_gap() {
    let corpus = Corpus::new();
    let port = FakePort::scores(vec![0.9, 0.5, 0.1]);
    let configuration = SearchConfiguration {
        section_prior: prior(0.5),
        ..bounded()
    };
    let mut admitted = corpus.admitted("run a task", configuration);
    admitted.cutoffs.setup = Instant::now() + admitted.cutoffs.window;
    let ranking = corpus.rank(&port, &admitted, "run a task").await;
    assert_eq!(texts(&ranking), corpus.prepared());
    assert_eq!(order(&ranking), [corpus.id(0), corpus.id(1), corpus.id(2)]);
    assert_eq!(ranking.fallbacks, order(&ranking));
    assert_eq!(ranking.context_unavailable, 3);
    assert_eq!(
        ranking.context_gap().unwrap(),
        "reranker context unavailable for 3 candidates; their chunk text was used"
    );
}

/// Classifies documents whose path starts with `prefix` as community pages
/// and every other example page as official documentation.
fn community(prefix: &str) -> Arc<dyn SourceClassifier> {
    let table = format!(
        r#"{{"schema": "maestro-source-classes/1", "rules": [
            {{"host": "example.org", "path_prefix": "/{prefix}", "class": "community",
              "label": "Community"}},
            {{"host": "example.org", "class": "official_docs", "label": "Docs"}}
        ]}}"#
    );
    Arc::new(SourceClassTable::parse(table.as_bytes()).unwrap())
}

fn official_first(weight: f32) -> SourcePrior {
    let mut classes = SourceClassSet::default();
    assert!(classes.insert("community"));
    SourcePrior::Soft { weight, classes }
}

#[tokio::test]
async fn the_source_prior_ranks_an_official_page_before_a_community_page() {
    let corpus = Corpus::new();
    let (release, intro, steps) = (corpus.id(0), corpus.id(1), corpus.id(2));
    let port = FakePort::scores(vec![0.9, 0.5, 0.1]);
    let configuration = SearchConfiguration {
        source_prior: official_first(0.5),
        ..SearchConfiguration::default()
    };
    let mut admitted = corpus.admitted("run a task", configuration);
    admitted.source_classes = Some(community("release"));
    let demoted = corpus.rank(&port, &admitted, "run a task").await;
    assert_eq!(order(&demoted), [intro, release, steps]);
    assert_eq!(
        texts(&demoted),
        [1, 0, 2].map(|index| corpus.prepared()[index].clone())
    );
    assert_eq!(top_rerank_score(&demoted.ranked), Some(0.9));

    let capped = SearchConfiguration {
        rerank_demotion_cap: Some(0),
        ..configuration
    };
    admitted.configuration = capped;
    let bounded_demotion = corpus.rank(&port, &admitted, "run a task").await;
    assert_eq!(order(&bounded_demotion), [release, intro, steps]);
}

#[tokio::test]
async fn without_a_table_or_with_the_prior_off_the_order_is_unchanged() {
    let corpus = Corpus::new();
    let fused = [corpus.id(0), corpus.id(1), corpus.id(2)];
    for (prior, table) in [
        (SourcePrior::Off, Some(community("release"))),
        (official_first(0.5), None),
        (SourcePrior::default(), None),
    ] {
        let port = FakePort::scores(vec![0.9, 0.5, 0.1]);
        let configuration = SearchConfiguration {
            source_prior: prior,
            ..SearchConfiguration::default()
        };
        let mut admitted = corpus.admitted("run a task", configuration);
        admitted.source_classes = table;
        let ranking = corpus.rank(&port, &admitted, "run a task").await;
        assert_eq!(order(&ranking), fused);
        assert_eq!(ranking.context_gap(), None);
    }
}

#[tokio::test]
async fn the_source_prior_classifies_only_within_the_rerank_depth() {
    let corpus = Corpus::new();
    let (release, intro, steps) = (corpus.id(0), corpus.id(1), corpus.id(2));
    let configuration = SearchConfiguration {
        rerank_depth: NonZeroUsize::new(2).unwrap(),
        source_prior: official_first(0.9),
        ..SearchConfiguration::default()
    };
    let port = FakePort::scores(vec![0.9, 0.5]);
    let mut admitted = corpus.admitted("run a task", configuration);
    admitted.source_classes = Some(community("guide"));
    let ranking = corpus.rank(&port, &admitted, "run a task").await;
    assert_eq!(order(&ranking), [release, steps, intro]);
}

#[tokio::test]
async fn source_classification_past_the_enrichment_cutoff_penalizes_nothing() {
    let corpus = Corpus::new();
    let port = FakePort::scores(vec![0.9, 0.5, 0.1]);
    let configuration = SearchConfiguration {
        source_prior: official_first(0.5),
        ..SearchConfiguration::default()
    };
    let mut admitted = corpus.admitted("run a task", configuration);
    admitted.source_classes = Some(community("release"));
    admitted.cutoffs.setup = Instant::now() + admitted.cutoffs.window;
    let ranking = corpus.rank(&port, &admitted, "run a task").await;
    assert_eq!(order(&ranking), [corpus.id(0), corpus.id(1), corpus.id(2)]);
    assert_eq!(ranking.status, RouteStatus::Ok);
}

fn evidence_input(corpus: &Corpus, admitted: AdmittedSearch, ranked: Vec<Ranked>) -> EvidenceInput {
    EvidenceInput {
        evidence: EvidenceSettings::default(),
        generation: admitted.generation,
        query: "run a task".to_owned(),
        understood: admitted.understood,
        version: None,
        principal: admitted.principal,
        scopes: Arc::new(corpus.scopes.clone()),
        ranked,
        observations: SearchObservations::default(),
        routes: BTreeMap::from([
            ("lexical".to_owned(), RouteStatus::Ok),
            ("rerank".to_owned(), RouteStatus::Ok),
        ]),
        inventory: None,
        budget: RequestBudget::default(),
        deadline: Instant::now() + Duration::from_secs(10),
        known_gaps: Vec::new(),
    }
}
