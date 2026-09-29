//! Compound questions through `search`: the whole question ranks as before,
//! each part ranks against its own words under the one request deadline and
//! scope snapshot, and a part that fails leaves the whole result.

use super::{
    configured_search::{Published, clean, published},
    models,
};
use maestro_kernel::{
    evidence::{RequestBudget, RouteStatus},
    gateway::{ChatRequest, Error, ModelCard, ModelPort, Role, Room},
    scope::{Right, Scope},
    store::Database,
};
use maestro_knowledge::{
    query::Understood,
    search::{
        EvidenceInput, ExpansionFailure, ExpansionFuture, Glossary, PartRecord, PartsRecord,
        QueryExpander, QuestionParts, Reranker, SearchConfiguration, SearchContext, SearchError,
        SearchRequest, Unsplit, routes::dense::Embedder, search,
    },
};
use std::{
    fmt::Debug,
    future::{Future, pending, ready},
    sync::{Arc, Mutex},
};
use tokio::time::{Duration, Instant};

/// A compound question: "how does the scheduler retry a job" and "only
/// after unrelated frequency padding appears".
const COMPOUND: &str =
    "how does the scheduler retry a job only after unrelated frequency padding appears";
/// Its parts, the second led by the topic.
const PARTS: [&str; 2] = [
    "how does the scheduler retry a job",
    "scheduler retry only after unrelated frequency padding appears",
];

/// Delegates embedding to the fixture's fake and reranks by shared words.
struct PartsPort<'a> {
    /// The fixture's embedder.
    inner: &'a models::Embedder,
    /// The question as asked.
    whole: &'static str,
    /// Every rerank query so far.
    queries: Mutex<Vec<String>>,
    /// Refuses to embed anything but the whole question.
    fail_part_embeds: bool,
    /// Never returns from a rerank of anything but the whole question.
    hang_part_reranks: bool,
}

impl<'a> PartsPort<'a> {
    /// A port over the fixture's embedder for `whole`.
    fn new(fixture: &'a Published, whole: &'static str) -> Self {
        Self {
            inner: &fixture.port,
            whole,
            queries: Mutex::new(Vec::new()),
            fail_part_embeds: false,
            hang_part_reranks: false,
        }
    }

    /// The rerank queries so far, sorted.
    fn queries(&self) -> Vec<String> {
        let mut queries = self.queries.lock().unwrap().clone();
        queries.sort();
        queries
    }
}

/// The lower-case words of four letters or more of `text`.
fn words(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| word.len() >= 4)
        .map(str::to_lowercase)
        .collect()
}

impl ModelPort for PartsPort<'_> {
    async fn embed(
        &self,
        card: &ModelCard,
        room: Room,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, Error> {
        if self.fail_part_embeds && inputs.iter().any(|input| input != self.whole) {
            return Err(Error::Unavailable {
                reason: "no room".to_owned(),
            });
        }
        self.inner.embed(card, room, inputs).await
    }
    fn rerank(
        &self,
        _card: &ModelCard,
        _room: Room,
        query: &str,
        documents: &[String],
    ) -> impl Future<Output = Result<Vec<f64>, Error>> + Send {
        self.queries.lock().unwrap().push(query.to_owned());
        let hang = self.hang_part_reranks && query != self.whole;
        let asked = words(query);
        let scores = documents
            .iter()
            .map(|document| {
                let held = words(document);
                f64::from(
                    u32::try_from(asked.iter().filter(|word| held.contains(word)).count()).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        async move {
            if hang {
                pending::<()>().await;
            }
            Ok(scores)
        }
    }
    async fn tokenize(&self, card: &ModelCard, room: Room, text: &str) -> Result<Vec<u32>, Error> {
        self.inner.tokenize(card, room, text).await
    }
    fn chat(
        &self,
        _: &ModelCard,
        _: Room,
        _: &ChatRequest,
    ) -> impl Future<Output = Result<String, Error>> + Send {
        ready(Err(Error::Unavailable {
            reason: "no chat".to_owned(),
        }))
    }
}

/// A bridge that never ends.
struct Hanging;

impl QueryExpander for Hanging {
    fn expand<'a>(&'a self, _: &'a Understood) -> ExpansionFuture<'a> {
        Box::pin(pending())
    }
}

/// A bridge that revokes the tester's grant, then adds nothing.
struct Revoking(Arc<Database>);

impl QueryExpander for Revoking {
    fn expand<'a>(&'a self, _: &'a Understood) -> ExpansionFuture<'a> {
        let workspace: Scope = "workspace/default".parse().unwrap();
        self.0
            .revoke("tester", &workspace, Right::Read, "test")
            .unwrap();
        Box::pin(async { Err(ExpansionFailure::NoMatch) })
    }
}

/// Searches `text` under `parts` and `configuration`, within `deadline_ms`,
/// with `port` serving every model and `bridge` bridging each part.
async fn run<'a>(
    fixture: &'a Published,
    port: &'a PartsPort<'a>,
    (text, parts): (&str, QuestionParts),
    bridge: Option<Arc<dyn QueryExpander + 'a>>,
    (configuration, deadline_ms): (SearchConfiguration, u32),
) -> Result<EvidenceInput, SearchError> {
    let reranker = models::card(Role::Reranker, 3);
    let context = SearchContext {
        database: fixture.kernel.database.clone(),
        principal: "tester",
        qdrant: &fixture.qdrant,
        embedder: Some(Embedder {
            port,
            card: &fixture.embedder_card,
        }),
        intent_expander: None,
        reranker: Some(Reranker {
            port,
            card: &reranker,
        }),
        source_classes: None,
        question_splitter: None,
        part_bridge: bridge,
    };
    let budget = RequestBudget {
        deadline_ms,
        ..RequestBudget::default()
    };
    let request = SearchRequest {
        configuration: SearchConfiguration {
            question_parts: parts,
            ..configuration
        },
        ..SearchRequest::new(&fixture.kernel.collection, text, None, budget)
    };
    Box::pin(search(&context, &request)).await
}

/// `input` without its parts record and deadline, for comparison.
fn whole_view(input: &EvidenceInput) -> impl PartialEq + Debug {
    let mut observations = input.observations.clone();
    observations.question_parts = None;
    (
        input.ranked.clone(),
        input.routes.clone(),
        input.known_gaps.clone(),
        input.query.clone(),
        observations,
    )
}

/// The part records of `input`.
fn records(input: &EvidenceInput) -> &[PartRecord] {
    &input.observations.question_parts.as_ref().unwrap().parts
}

#[tokio::test]
async fn off_and_a_one_part_question_rank_exactly_as_before() {
    let fixture = published().await;
    let question = "how does the scheduler retry a job";
    let port = PartsPort::new(&fixture, question);
    let default = SearchConfiguration::default();
    let off = run(
        &fixture,
        &port,
        (question, QuestionParts::Off),
        None,
        (default, 5000),
    )
    .await
    .unwrap();
    assert_eq!(off.observations.question_parts, None);
    let split = run(
        &fixture,
        &port,
        (question, QuestionParts::Split),
        None,
        (default, 5000),
    )
    .await
    .unwrap();
    assert_eq!(whole_view(&split), whole_view(&off));
    assert_eq!(
        split.observations.question_parts,
        Some(PartsRecord {
            whole: None,
            parts: Vec::new(),
            unsplit: Some(Unsplit::NoRelation),
        })
    );
    assert_eq!(port.queries(), [question, question]);
    clean(&fixture).await;
}

#[tokio::test]
async fn each_part_is_reranked_against_its_own_words_beside_the_whole_question() {
    let fixture = published().await;
    let port = PartsPort::new(&fixture, COMPOUND);
    let default = SearchConfiguration::default();
    let off = run(
        &fixture,
        &port,
        (COMPOUND, QuestionParts::Off),
        None,
        (default, 5000),
    )
    .await
    .unwrap();
    let split = run(
        &fixture,
        &port,
        (COMPOUND, QuestionParts::Split),
        None,
        (default, 5000),
    )
    .await
    .unwrap();
    assert_eq!(split.query, COMPOUND);
    assert_eq!(whole_view(&split), whole_view(&off));
    let mut expected = vec![COMPOUND, COMPOUND, PARTS[0], PARTS[1]];
    expected.sort_unstable();
    assert_eq!(port.queries(), expected);
    assert_eq!(
        split
            .observations
            .question_parts
            .as_ref()
            .unwrap()
            .whole
            .as_ref(),
        Some(&split.ranked[0].candidate.fused.chunk_id)
    );
    let records = records(&split);
    assert_eq!(
        records
            .iter()
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>(),
        PARTS
    );
    for part in records {
        assert_eq!(part.status, RouteStatus::Ok);
        assert_eq!(part.bridge, "");
        assert_eq!(part.bridge_status, None);
    }
    let first = records[0].best.as_deref().unwrap();
    assert!(!first.contains("filler"), "{first}");
    let second = records[1].best.as_deref().unwrap();
    assert!(second.contains("frequency-filler"), "{second}");
    clean(&fixture).await;
}

#[tokio::test]
async fn a_bridge_adds_documentation_words_to_the_parts_it_matches() {
    let fixture = published().await;
    let port = PartsPort::new(&fixture, COMPOUND);
    let glossary = Glossary::parse(
        br#"{"schema": "maestro-glossary/1", "entries": [
            {"id": "G1", "phrases": ["frequency padding"], "add": ["Logs"]}
        ]}"#,
    )
    .unwrap();
    let split = run(
        &fixture,
        &port,
        (COMPOUND, QuestionParts::Split),
        Some(Arc::new(glossary)),
        (SearchConfiguration::default(), 5000),
    )
    .await
    .unwrap();
    let records = records(&split);
    assert_eq!(
        records[0].bridge_status,
        Some(RouteStatus::Unavailable("intent_no_match".to_owned()))
    );
    assert_eq!(records[1].bridge, "Logs");
    assert_eq!(records[1].bridge_status, Some(RouteStatus::Ok));
    assert!(port.queries().contains(&format!("{} Logs", PARTS[1])));
    assert_eq!(split.query, COMPOUND);
    clean(&fixture).await;
}

#[tokio::test]
async fn a_part_whose_routes_fail_leaves_the_whole_question_result() {
    let fixture = published().await;
    let mut port = PartsPort::new(&fixture, COMPOUND);
    port.fail_part_embeds = true;
    let dense_only = SearchConfiguration {
        lexical_enabled: false,
        identifier_enabled: false,
        structured_enabled: false,
        ..SearchConfiguration::default()
    };
    let off = run(
        &fixture,
        &port,
        (COMPOUND, QuestionParts::Off),
        None,
        (dense_only, 5000),
    )
    .await
    .unwrap();
    let split = run(
        &fixture,
        &port,
        (COMPOUND, QuestionParts::Split),
        None,
        (dense_only, 5000),
    )
    .await
    .unwrap();
    assert_eq!(whole_view(&split), whole_view(&off));
    assert_eq!(records(&split).len(), 2);
    for part in records(&split) {
        assert_eq!(
            part.status,
            RouteStatus::Unavailable("part_ranked_nothing".to_owned())
        );
        assert_eq!(part.best, None);
    }
    clean(&fixture).await;
}

#[tokio::test]
async fn every_part_ends_within_the_one_request_deadline() {
    let fixture = published().await;
    let mut port = PartsPort::new(&fixture, COMPOUND);
    let default = SearchConfiguration::default();
    let off = run(
        &fixture,
        &port,
        (COMPOUND, QuestionParts::Off),
        None,
        (default, 1500),
    )
    .await
    .unwrap();
    port.hang_part_reranks = true;
    let started = Instant::now();
    let split = run(
        &fixture,
        &port,
        (COMPOUND, QuestionParts::Split),
        Some(Arc::new(Hanging)),
        (default, 1500),
    )
    .await
    .unwrap();
    assert!(started.elapsed() < Duration::from_millis(1500));
    assert_eq!(whole_view(&split), whole_view(&off));
    assert_eq!(records(&split).len(), 2);
    for part in records(&split) {
        assert_eq!(
            part.bridge_status,
            Some(RouteStatus::Unavailable(
                "intent_deadline_exceeded".to_owned()
            ))
        );
        assert_eq!(part.status, RouteStatus::Ok);
        assert!(part.best.is_some());
    }
    clean(&fixture).await;
}

#[tokio::test]
async fn a_grant_revoked_during_a_part_fails_the_search() {
    let fixture = published().await;
    let port = PartsPort::new(&fixture, COMPOUND);
    let result = run(
        &fixture,
        &port,
        (COMPOUND, QuestionParts::Split),
        Some(Arc::new(Revoking(fixture.kernel.database.clone()))),
        (SearchConfiguration::default(), 5000),
    )
    .await;
    assert!(matches!(result, Err(SearchError::PermissionsChanged)));
    clean(&fixture).await;
}
