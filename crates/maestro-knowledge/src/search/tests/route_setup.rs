//! A route's one-time setup, such as loading its model, is not route time.

use super::{rerank::card, support::CandidateDb};
use crate::{
    index::{Qdrant, embedding_profile},
    search::{
        DISABLED_BY_CONFIGURATION, Query, deadline::from_budget, route_execution::dense_outcome,
        routes::dense::Embedder,
    },
};
use maestro_kernel::{
    evidence::{RequestBudget, RouteStatus},
    gateway::{ChatRequest, Error, ModelCard, ModelPort, Role, Room},
};
use std::{
    future::{self, Future},
    sync::atomic::{AtomicBool, Ordering},
};
use tokio::time::{Duration, Instant, sleep};

/// An embedder whose model takes `setup` on the paused clock to load, at
/// its setup or else at its first call, as a router's card check does, and
/// whose embedding is then refused, or never answers when `hangs`, so that
/// the route ends without Qdrant.
struct LoadingEmbedder {
    /// How long its model takes to load.
    setup: Duration,
    /// Whether its embedding never answers.
    hangs: bool,
    /// Whether its setup is refused once its model loaded.
    refuses_setup: bool,
    /// Whether its model finished loading.
    loaded: AtomicBool,
}

impl LoadingEmbedder {
    /// Loads the model unless it is loaded.
    async fn load(&self) {
        if !self.loaded.load(Ordering::Relaxed) {
            sleep(self.setup).await;
            self.loaded.store(true, Ordering::Relaxed);
        }
    }
}

impl ModelPort for LoadingEmbedder {
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
        self.load().await;
        if self.hangs {
            future::pending::<()>().await;
        }
        Err(Error::Unavailable {
            reason: "refused once loaded".to_owned(),
        })
    }

    fn rerank(
        &self,
        _card: &ModelCard,
        _room: Room,
        _query: &str,
        _documents: &[String],
    ) -> impl Future<Output = Result<Vec<f64>, Error>> + Send {
        future::ready(Ok(Vec::new()))
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
    let port = LoadingEmbedder {
        setup,
        hangs,
        refuses_setup,
        loaded: AtomicBool::new(false),
    };
    let embedder = Embedder {
        port: &port,
        card: &embedder_card,
    };
    let started = Instant::now();
    let cutoffs = from_budget(
        started,
        RequestBudget {
            deadline_ms: 1500,
            ..RequestBudget::default()
        },
    );
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
        dense_after_setup(Duration::from_millis(800), false).await,
        (
            RouteStatus::Unavailable("embedder unavailable".to_owned()),
            Duration::from_millis(800)
        )
    );
    assert_eq!(
        dense_after_setup(Duration::from_millis(800), true).await,
        (
            RouteStatus::Unavailable("deadline_exceeded".to_owned()),
            Duration::from_millis(1100)
        )
    );
}

#[tokio::test(start_paused = true)]
async fn a_setup_past_its_bound_leaves_the_route_one_window_after_the_bound() {
    assert_eq!(
        dense_after_setup(Duration::from_millis(1500), false).await,
        (
            RouteStatus::Unavailable("deadline_exceeded".to_owned()),
            Duration::from_millis(1150)
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
