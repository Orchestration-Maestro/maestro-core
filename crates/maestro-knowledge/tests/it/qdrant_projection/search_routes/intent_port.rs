//! A model port whose expansion chat and reranker a test scripts, over the
//! fixture's deterministic embedder.

use super::{configured_search::Published, models};
use maestro_kernel::{
    evidence::RequestBudget,
    gateway::{ChatRequest, Error, ModelCard, ModelPort, Role, Room},
    scope::{Right, Scope},
    store::Database,
};
use maestro_knowledge::search::{
    EvidenceInput, HydeExpander, Reranker, SearchConfiguration, SearchContext, SearchError,
    SearchRequest, routes::dense::Embedder, search,
};
use std::{
    future::{Future, pending},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::time::Duration;

/// How one rerank call of the port ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RerankFault {
    /// The router refuses the call.
    Fails,
    /// The call never returns.
    Hangs,
}

/// Delegates retrieval to the fixture's fake and scripts chat and rerank.
pub(super) struct IntentPort<'a> {
    pub(super) inner: &'a models::Embedder,
    /// Chat calls so far.
    pub(super) calls: AtomicUsize,
    /// Rerank calls so far.
    pub(super) rerank_calls: AtomicUsize,
    /// The chat reply; none refuses the call.
    pub(super) reply: Option<&'a str>,
    /// A nonzero delay holds every chat call open.
    pub(super) delay: Duration,
    /// The rerank call, counted from 0, that goes wrong, and how.
    pub(super) rerank_fault: Option<(usize, RerankFault)>,
    /// Scores each document by its text instead of 1.0 each.
    pub(super) score_of: Option<fn(&str) -> f64>,
    /// Revokes the tester's workspace grant when chat is called.
    pub(super) revoke: Option<Arc<Database>>,
    /// The last chat request.
    pub(super) request: Mutex<Option<ChatRequest>>,
    /// How many documents each rerank call scored.
    pub(super) reranked: Mutex<Vec<usize>>,
}

impl<'a> IntentPort<'a> {
    /// A port over `inner` whose chat answers `reply`.
    pub(super) fn new(inner: &'a models::Embedder, reply: Option<&'a str>) -> Self {
        Self {
            inner,
            calls: AtomicUsize::new(0),
            rerank_calls: AtomicUsize::new(0),
            reply,
            delay: Duration::ZERO,
            rerank_fault: None,
            score_of: None,
            revoke: None,
            request: Mutex::new(None),
            reranked: Mutex::new(Vec::new()),
        }
    }
}

impl ModelPort for IntentPort<'_> {
    async fn embed(
        &self,
        card: &ModelCard,
        room: Room,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, Error> {
        self.inner.embed(card, room, inputs).await
    }
    fn rerank(
        &self,
        _card: &ModelCard,
        room: Room,
        _query: &str,
        documents: &[String],
    ) -> impl Future<Output = Result<Vec<f64>, Error>> + Send {
        assert_eq!(room, Room::Free);
        let call = self.rerank_calls.fetch_add(1, Ordering::SeqCst);
        self.reranked.lock().unwrap().push(documents.len());
        let fault = self
            .rerank_fault
            .and_then(|(at, fault)| (at == call).then_some(fault));
        let scores = documents
            .iter()
            .map(|document| self.score_of.map_or(1.0, |score_of| score_of(document)))
            .collect::<Vec<_>>();
        async move {
            match fault {
                Some(RerankFault::Fails) => Err(Error::Unavailable {
                    reason: "no room".to_owned(),
                }),
                Some(RerankFault::Hangs) => pending().await,
                None => Ok(scores),
            }
        }
    }
    async fn tokenize(&self, card: &ModelCard, room: Room, text: &str) -> Result<Vec<u32>, Error> {
        self.inner.tokenize(card, room, text).await
    }
    async fn chat(
        &self,
        _card: &ModelCard,
        room: Room,
        request: &ChatRequest,
    ) -> Result<String, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(room, Room::Free);
        *self.request.lock().unwrap() = Some(request.clone());
        if let Some(database) = &self.revoke {
            let workspace: Scope = "workspace/default".parse().unwrap();
            database
                .revoke("tester", &workspace, Right::Read, "test")
                .unwrap();
        }
        if !self.delay.is_zero() {
            pending::<()>().await;
        }
        self.reply
            .map(str::to_owned)
            .ok_or_else(|| Error::Unavailable {
                reason: "no room".to_owned(),
            })
    }
}

/// Searches `text` in the fixture under `configuration` and `budget`, with
/// `port` serving every model.
pub(super) async fn search_with(
    fixture: &Published,
    port: &IntentPort<'_>,
    text: &str,
    configuration: SearchConfiguration,
    budget: RequestBudget,
) -> Result<EvidenceInput, SearchError> {
    let card = models::card(Role::Answerer, 3);
    let reranker = models::card(Role::Reranker, 3);
    let context = SearchContext {
        database: fixture.kernel.database.clone(),
        principal: "tester",
        projection: &fixture.qdrant,
        embedder: Some(Embedder {
            port,
            card: &fixture.embedder_card,
        }),
        intent_expander: Some(Box::new(HydeExpander::new(port, &card).unwrap())),
        reranker: Some(Reranker {
            port,
            card: &reranker,
        }),
        source_classes: None,
    };
    let request = SearchRequest {
        configuration,
        ..SearchRequest::new(&fixture.kernel.collection, text, None, budget)
    };
    Box::pin(search(&context, &request)).await
}

/// Searches `text` under `configuration` within the default budget.
pub(super) async fn run_configured(
    fixture: &Published,
    port: &IntentPort<'_>,
    text: &str,
    configuration: SearchConfiguration,
) -> EvidenceInput {
    search_with(fixture, port, text, configuration, RequestBudget::default())
        .await
        .unwrap()
}
