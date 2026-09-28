//! A route's one-time setup, such as loading its model, is not route time,
//! and a model still loading at the setup bound costs only its own stage.

use super::{
    rerank::{candidate, card},
    support::CandidateDb,
};
use crate::{
    index::{Qdrant, embedding_profile},
    query::understand,
    search::{
        DEADLINE_EXCEEDED, DISABLED_BY_CONFIGURATION, Query, Reranker,
        deadline::{Deadlines, from_budget},
        rerank::rerank_candidates,
        route_execution::{dense_outcome, prepare_reranker},
        routes::dense::Embedder,
    },
};
use maestro_kernel::{
    evidence::{RequestBudget, RouteStatus},
    gateway::{ChatRequest, Error, ModelCard, ModelPort, Role, Room},
};
use std::{
    future::{self, Future},
    num::NonZeroUsize,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio::time::{Duration, Instant, sleep};

/// A model whose loading takes `setup` on the paused clock, at its setup or
/// else at its first call, as a router's card check does, and whose setup
/// may then be refused. Once loaded, its embedding is refused, so that the
/// dense route ends without Qdrant, and its reranking scores every
/// document; either never answers when `hangs`.
struct LoadingModel {
    /// How long its model takes to load.
    setup: Duration,
    /// Whether its embedding or reranking never answers.
    hangs: bool,
    /// Whether its setup is refused once its model loaded.
    refuses_setup: bool,
    /// Whether its model finished loading.
    loaded: AtomicBool,
    /// How many reranking calls it received.
    reranks: AtomicUsize,
}

impl LoadingModel {
    /// A model that takes `setup` to load and then answers, or never
    /// answers when `hangs`, and whose setup is refused when `refuses_setup`.
    const fn new(setup: Duration, hangs: bool, refuses_setup: bool) -> Self {
        Self {
            setup,
            hangs,
            refuses_setup,
            loaded: AtomicBool::new(false),
            reranks: AtomicUsize::new(0),
        }
    }

    /// Loads the model unless it is loaded.
    async fn load(&self) {
        if !self.loaded.load(Ordering::Relaxed) {
            sleep(self.setup).await;
            self.loaded.store(true, Ordering::Relaxed);
        }
    }

    /// Loads the model, then never returns when it hangs.
    async fn answer(&self) {
        self.load().await;
        if self.hangs {
            future::pending::<()>().await;
        }
    }
}

impl ModelPort for LoadingModel {
    async fn prepare(&self, _card: &ModelCard, _room: Room) -> Result<(), Error> {
        self.load().await;
        if self.refuses_setup {
            return Err(Error::Unavailable {
                reason: "no free room".to_owned(),
            });
        }
        Ok(())
    }

    async fn embed(
        &self,
        _card: &ModelCard,
        _room: Room,
        _inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, Error> {
        self.answer().await;
        Err(Error::Unavailable {
            reason: "refused once loaded".to_owned(),
        })
    }

    async fn rerank(
        &self,
        _card: &ModelCard,
        _room: Room,
        _query: &str,
        documents: &[String],
    ) -> Result<Vec<f64>, Error> {
        self.reranks.fetch_add(1, Ordering::Relaxed);
        self.answer().await;
        Ok(vec![1.0; documents.len()])
    }

    fn tokenize(
        &self,
        _card: &ModelCard,
        _room: Room,
        _text: &str,
    ) -> impl Future<Output = Result<Vec<u32>, Error>> + Send {
        future::ready(Ok(Vec::new()))
    }

    fn chat(
        &self,
        _card: &ModelCard,
        _room: Room,
        _request: &ChatRequest,
    ) -> impl Future<Output = Result<String, Error>> + Send {
        future::ready(Ok(String::new()))
    }
}

/// The dense status after an embedder setup of `setup` under a 1.5 s
/// budget, and when the route ended after the request started.
async fn dense_after_setup(setup: Duration, hangs: bool) -> (RouteStatus, Duration) {
    dense_route(true, setup, hangs, false).await.0
}

/// The dense route's status and end time, when `enabled`, and whether its
/// embedder's model was loaded.
async fn dense_route(
    enabled: bool,
    setup: Duration,
    hangs: bool,
    refuses_setup: bool,
) -> ((RouteStatus, Duration), bool) {
    let mut fixture = CandidateDb::new(b"prepared text", "docs");
    let embedder_card = card(Role::Embedder, 128);
    fixture.generation.embedding_profile = embedding_profile(&embedder_card);
    let qdrant = Qdrant::new("http://127.0.0.1:1").unwrap();
    let query = Query {
        generation: &fixture.generation,
        scopes: &fixture.scopes,
        text: "ERR-042",
        limit: 10,
        version: None,
        qdrant: &qdrant,
    };
    let port = LoadingModel::new(setup, hangs, refuses_setup);
    let embedder = Embedder {
        port: &port,
        card: &embedder_card,
    };
    let (started, cutoffs) = default_search_cutoffs();
    let status = dense_outcome(enabled, &query, Some(&embedder), &cutoffs)
        .await
        .status;
    (
        (status, started.elapsed()),
        port.loaded.load(Ordering::Relaxed),
    )
}

#[tokio::test(start_paused = true)]
async fn a_disabled_dense_route_does_not_prepare_its_model() {
    assert_eq!(
        dense_route(false, Duration::from_millis(800), false, false).await,
        (
            (
                RouteStatus::Unavailable(DISABLED_BY_CONFIGURATION.to_owned()),
                Duration::ZERO
            ),
            false
        )
    );
}

#[tokio::test(start_paused = true)]
async fn a_setup_longer_than_the_route_window_leaves_the_route_its_window() {
    assert_eq!(
        dense_after_setup(Duration::from_millis(500), false).await,
        (
            RouteStatus::Unavailable("embedder unavailable".to_owned()),
            Duration::from_millis(500)
        )
    );
    assert_eq!(
        dense_after_setup(Duration::from_millis(500), true).await,
        (
            RouteStatus::Unavailable("deadline_exceeded".to_owned()),
            Duration::from_millis(800)
        )
    );
}

#[tokio::test(start_paused = true)]
async fn a_late_setup_leaves_the_route_only_until_the_bound() {
    assert_eq!(
        dense_after_setup(Duration::from_millis(800), true).await,
        (
            RouteStatus::Unavailable("deadline_exceeded".to_owned()),
            Duration::from_millis(850)
        )
    );
}

#[tokio::test(start_paused = true)]
async fn a_setup_ending_at_the_bound_leaves_the_route_no_call() {
    assert_eq!(
        dense_after_setup(Duration::from_millis(850), false).await,
        (
            RouteStatus::Unavailable("deadline_exceeded".to_owned()),
            Duration::from_millis(850)
        )
    );
}

// The route used to get a window after the bound, ending at 1150 ms, which
// left evidence assembly 350 ms: it took 340-500 ms on a real collection.
#[tokio::test(start_paused = true)]
async fn a_setup_past_its_bound_drops_the_route_at_the_bound() {
    assert_eq!(
        dense_after_setup(Duration::from_millis(1500), false).await,
        (
            RouteStatus::Unavailable("deadline_exceeded".to_owned()),
            Duration::from_millis(850)
        )
    );
}

#[tokio::test(start_paused = true)]
async fn a_refused_setup_leaves_the_route_unavailable_at_once() {
    assert_eq!(
        dense_route(true, Duration::from_millis(100), true, true)
            .await
            .0,
        (
            RouteStatus::Unavailable("embedder unavailable".to_owned()),
            Duration::from_millis(100)
        )
    );
}

/// The request start and cutoffs of a search under a 1.5 s budget.
fn default_search_cutoffs() -> (Instant, Deadlines) {
    let started = Instant::now();
    let cutoffs = from_budget(
        started,
        RequestBudget {
            deadline_ms: 1500,
            ..RequestBudget::default()
        },
    );
    (started, cutoffs)
}

/// What a search under a 1.5 s budget reranks with a reranker of `role`,
/// when enabled, whose model takes `setup` to load and then answers or,
/// when it `hangs`, never answers: the rerank status, when the rerank ended
/// after the request started, and how many reranking calls the model got.
async fn rerank_after_setup(
    role: Role,
    enabled: bool,
    setup: Duration,
    hangs: bool,
) -> (RouteStatus, Duration, usize) {
    let reranker_card = card(role, 128);
    let port = LoadingModel::new(setup, hangs, false);
    let reranker = Reranker {
        port: &port,
        card: &reranker_card,
    };
    let (started, cutoffs) = default_search_cutoffs();
    prepare_reranker(Some(&reranker), enabled, &cutoffs).await;
    let (_, status) = rerank_candidates(
        &understand("ERR-042"),
        vec![candidate("candidate", 1.0, "prepared text")],
        Some(&reranker),
        enabled.then_some(NonZeroUsize::MIN),
        cutoffs.setup,
    )
    .await;
    (
        status,
        started.elapsed(),
        port.reranks.load(Ordering::Relaxed),
    )
}

#[tokio::test(start_paused = true)]
async fn a_reranker_still_loading_at_the_setup_bound_loses_only_the_rerank() {
    assert_eq!(
        rerank_after_setup(Role::Reranker, true, Duration::from_secs(5), false).await,
        (
            RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned()),
            Duration::from_millis(850),
            0
        )
    );
}

#[tokio::test(start_paused = true)]
async fn a_reranker_loaded_within_its_bound_reranks_until_the_bound() {
    assert_eq!(
        rerank_after_setup(Role::Reranker, true, Duration::from_millis(600), false).await,
        (RouteStatus::Ok, Duration::from_millis(600), 1)
    );
    assert_eq!(
        rerank_after_setup(Role::Reranker, true, Duration::from_millis(600), true).await,
        (
            RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned()),
            Duration::from_millis(850),
            1
        )
    );
}

#[tokio::test(start_paused = true)]
async fn a_rerank_that_cannot_run_prepares_no_model() {
    let (started, cutoffs) = default_search_cutoffs();
    let embedder_card = card(Role::Embedder, 128);
    let port = LoadingModel::new(Duration::from_secs(5), false, false);
    let not_a_reranker = Reranker {
        port: &port,
        card: &embedder_card,
    };
    prepare_reranker(Some(&not_a_reranker), true, &cutoffs).await;
    prepare_reranker::<LoadingModel>(None, true, &cutoffs).await;
    assert_eq!(started.elapsed(), Duration::ZERO);
    assert!(!port.loaded.load(Ordering::Relaxed));
    assert_eq!(
        rerank_after_setup(Role::Reranker, false, Duration::from_secs(5), false).await,
        (
            RouteStatus::Unavailable(DISABLED_BY_CONFIGURATION.to_owned()),
            Duration::ZERO,
            0
        )
    );
}
