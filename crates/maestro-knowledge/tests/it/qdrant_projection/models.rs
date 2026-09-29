//! The embedder the tests publish through: the deterministic fake models of
//! the gateway, behind a port that records each embedding call and can
//! spoil the vectors of one call, or refuse it, as a real embedder might,
//! and whose reranker can be slow to load or to score.

use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{
        CardFields, CardIdentity, ChatRequest, Error, FakeModels, Limits, ModelCard, ModelPort,
        Role, Room, RouterEntry,
        card_v2::{Capability, TextFormat},
    },
};
use maestro_test_scratch::scratch_directory;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs, future,
    num::{NonZeroU32, NonZeroUsize},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::Notify, time};

/// The card of a model filling `role` from the router's entry `embed`, of
/// `dimensions` when it is an embedder, recorded in a store that is gone
/// once it is made.
pub(super) fn card(role: Role, dimensions: usize) -> ModelCard {
    let fields = CardFields {
        role,
        router_entry: RouterEntry::parse("embed").unwrap(),
        file_digest: Digest::of(b"model file"),
        template_digest: None,
        server_build: "b6500-3f2c9a1b".to_owned(),
        dimensions: (role == Role::Embedder).then(|| NonZeroUsize::new(dimensions).unwrap()),
        limits: Limits {
            context_tokens: NonZeroU32::new(8192).unwrap(),
            output_tokens: None,
        },
        suite_results: Vec::new(),
    };
    let root = scratch_directory().unwrap();
    let card = ModelCard::record(&Store::new(&root), &fields).unwrap();
    fs::remove_dir_all(&root).unwrap();
    card
}

/// An embedder's card of `dimensions`.
pub(super) fn embedder(dimensions: usize) -> ModelCard {
    card(Role::Embedder, dimensions)
}

/// A v2 card loaded from the public synthetic fixture.
pub(super) fn v2_embedder() -> ModelCard {
    v2_card_from_fixture(false)
}

/// The synthetic v2 card with a query instruction for dense-route tests.
pub(super) fn v2_embedder_with_query_prefix() -> ModelCard {
    v2_card_from_fixture(true)
}

fn v2_card_from_fixture(query_prefix: bool) -> ModelCard {
    let value: Value = serde_json::from_str(include_str!(
        "../../fixtures/synthetic/evals/model-card-v2.json"
    ))
    .unwrap();
    let mut identity: CardIdentity = serde_json::from_value(value["identity"].clone()).unwrap();
    if query_prefix {
        identity.formats.query = Capability::Supported(TextFormat {
            prefix: "Instruct: retrieve relevant passages\nQuery: ".to_owned(),
            suffix: String::new(),
        });
    }
    let root = scratch_directory().unwrap();
    let card = ModelCard::record_v2(&Store::new(&root), &identity).unwrap();
    fs::remove_dir_all(root).unwrap();
    card
}

/// How one embedding call goes wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Fault {
    /// The router has no free room for the embedder.
    Unavailable,
    /// The first vector has one dimension fewer than the card's.
    Dimensions,
    /// The first vector holds a NaN.
    NotANumber,
    /// The first vector holds an infinity.
    Infinite,
    /// The first vector is zero.
    Zero,
    /// One vector fewer than inputs comes back.
    Fewer,
}

/// What the port saw and was told to do.
#[derive(Debug, Default)]
struct Script {
    /// The inputs of each embedding call, in order.
    calls: Vec<Vec<String>>,
    /// The room each embedding call named, in order.
    rooms: Vec<Room>,
    /// The fault of the call of each number, counted from 0.
    faults: BTreeMap<usize, Fault>,
    /// Number of calls the fake reranker received.
    rerank_calls: usize,
    /// When each embedding call answers: late, as a loaded host's
    /// embedder does, or never.
    embedding_delay: Answers,
}

/// When an embedding call answers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Answers {
    /// At once.
    #[default]
    AtOnce,
    /// After this long, which it moves the test's stopped clock by.
    After(Duration),
    /// Never.
    Never,
}

/// Which model(s) never finish loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SlowModels {
    /// The embedder is still loading.
    Embedder,
    /// The reranker is still loading.
    Reranker,
    /// Both models are still loading.
    Both,
}

/// How its reranker is slow to load or to score.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SlowReranker {
    /// It never finishes loading, as one the router unloaded may not within
    /// a search: neither its setup nor a call returns.
    Loading,
    /// It loads at once, then never scores.
    Hanging,
}

/// The fake models, with each embedding call recorded, and a fault for the
/// calls told.
#[derive(Debug, Clone, Default)]
pub(super) struct Embedder {
    /// What it saw and was told.
    script: Arc<Mutex<Script>>,
    /// A gate that can pause a retrieval embedding call.
    gate: Option<Arc<Gate>>,
    /// Whether its embedder stays cold for the whole request.
    embed_never_ready: bool,
    /// How its reranker is slow, when it is.
    slow_reranker: Option<SlowReranker>,
}

#[derive(Debug, Default)]
struct Gate {
    started: Notify,
    release: Notify,
}

/// Controls one embedding call held by a test.
#[derive(Debug, Clone)]
pub(super) struct EmbedGate(Arc<Gate>);

impl EmbedGate {
    pub(super) async fn wait_started(&self) {
        self.0.started.notified().await;
    }

    pub(super) fn release(&self) {
        self.0.release.notify_one();
    }
}

impl Embedder {
    /// Creates a port that pauses its next embedding call until released.
    pub(super) fn gated() -> (Self, EmbedGate) {
        let gate = Arc::new(Gate::default());
        (
            Self {
                script: Arc::new(Mutex::new(Script::default())),
                gate: Some(gate.clone()),
                embed_never_ready: false,
                slow_reranker: None,
            },
            EmbedGate(gate),
        )
    }

    /// Creates a port whose reranker is `slow`.
    pub(super) fn with_slow_reranker(slow: SlowReranker) -> Self {
        Self {
            slow_reranker: Some(slow),
            ..Self::default()
        }
    }

    /// Creates a port whose selected model(s) never finish loading.
    pub(super) fn with_slow_models(slow: SlowModels) -> Self {
        Self {
            embed_never_ready: matches!(slow, SlowModels::Embedder | SlowModels::Both),
            slow_reranker: matches!(slow, SlowModels::Reranker | SlowModels::Both)
                .then_some(SlowReranker::Loading),
            ..Self::default()
        }
    }

    /// Waits until the test embedder is ready.
    async fn wait_for_embedder(&self) {
        if self.embed_never_ready {
            future::pending::<()>().await;
        }
    }

    /// Waits for the reranker to load or score when the test model is slow.
    async fn wait_for_reranker(&self, scoring: bool) {
        if self.slow_reranker == Some(SlowReranker::Loading)
            || (scoring && self.slow_reranker == Some(SlowReranker::Hanging))
        {
            future::pending::<()>().await;
        }
    }

    /// Makes every later embedding call answer as `delay` says.
    pub(super) fn delay_embeddings(&self, delay: Answers) {
        self.script.lock().unwrap().embedding_delay = delay;
    }

    /// The number of rerank calls made so far.
    pub(super) fn rerank_calls(&self) -> usize {
        self.script.lock().unwrap().rerank_calls
    }

    /// Makes the embedding call `call`, counted from 0 over the port's life,
    /// go wrong as `fault` says.
    pub(super) fn fail(&self, call: usize, fault: Fault) {
        self.script.lock().unwrap().faults.insert(call, fault);
    }

    /// The inputs of each embedding call so far, in order.
    pub(super) fn calls(&self) -> Vec<Vec<String>> {
        self.script.lock().unwrap().calls.clone()
    }

    /// The room each embedding call so far named, in order.
    pub(super) fn rooms(&self) -> Vec<Room> {
        self.script.lock().unwrap().rooms.clone()
    }
}

impl ModelPort for Embedder {
    async fn prepare(&self, card: &ModelCard, _room: Room) -> Result<(), Error> {
        match card.fields().role {
            Role::Embedder => self.wait_for_embedder().await,
            Role::Reranker => self.wait_for_reranker(false).await,
            Role::Answerer => {}
        }
        Ok(())
    }

    async fn embed(
        &self,
        card: &ModelCard,
        room: Room,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, Error> {
        let fault = {
            let mut script = self.script.lock().unwrap();
            let call = script.calls.len();
            script.calls.push(inputs.to_vec());
            script.rooms.push(room);
            script.faults.get(&call).copied()
        };
        let delay = self.script.lock().unwrap().embedding_delay;
        match delay {
            Answers::AtOnce => {}
            Answers::After(delay) => time::advance(delay).await,
            Answers::Never => future::pending().await,
        }
        let mut vectors = FakeModels.embed(card, room, inputs).await?;
        if let Some(gate) = &self.gate {
            gate.started.notify_one();
            gate.release.notified().await;
        }
        match fault {
            None => {}
            Some(Fault::Unavailable) => {
                return Err(Error::Unavailable {
                    reason: "loading it would unload a chat model".to_owned(),
                });
            }
            Some(Fault::Dimensions) => drop(vectors[0].pop()),
            Some(Fault::NotANumber) => vectors[0][0] = f32::NAN,
            Some(Fault::Infinite) => vectors[0][0] = f32::INFINITY,
            Some(Fault::Zero) => vectors[0].fill(0.0),
            Some(Fault::Fewer) => drop(vectors.pop()),
        }
        Ok(vectors)
    }

    async fn rerank(
        &self,
        card: &ModelCard,
        room: Room,
        query: &str,
        documents: &[String],
    ) -> Result<Vec<f64>, Error> {
        self.script.lock().unwrap().rerank_calls += 1;
        self.wait_for_reranker(true).await;
        FakeModels.rerank(card, room, query, documents).await
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
