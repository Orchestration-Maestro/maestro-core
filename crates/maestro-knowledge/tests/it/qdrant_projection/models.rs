//! The embedder the tests publish through: the deterministic fake models of
//! the gateway, behind a port that records each embedding call and can
//! spoil the vectors of one call, or refuse it, as a real embedder might.

use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{
        CardFields, Error, FakeModels, Limits, Message, ModelCard, ModelPort, Role, Room,
        RouterEntry,
    },
};
use std::{
    collections::BTreeMap,
    env, fs,
    num::{NonZeroU32, NonZeroUsize},
    process,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

/// The card of a model filling `role` from the router's entry `embed`, of
/// `dimensions` when it is an embedder, recorded in a store that is gone
/// once it is made.
pub(super) fn card(role: Role, dimensions: usize) -> ModelCard {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
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
    let root = env::temp_dir().join(format!(
        "maestro-knowledge-qdrant-card-{}-{}",
        process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let card = ModelCard::record(&Store::new(&root), &fields).unwrap();
    fs::remove_dir_all(&root).unwrap();
    card
}

/// An embedder's card of `dimensions`.
pub(super) fn embedder(dimensions: usize) -> ModelCard {
    card(Role::Embedder, dimensions)
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
}

/// The fake models, with each embedding call recorded, and a fault for the
/// calls told.
#[derive(Debug, Clone, Default)]
pub(super) struct Embedder {
    /// What it saw and was told.
    script: Arc<Mutex<Script>>,
}

impl Embedder {
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
        let mut vectors = FakeModels.embed(card, room, inputs).await?;
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
        FakeModels.rerank(card, room, query, documents).await
    }

    async fn tokenize(&self, card: &ModelCard, room: Room, text: &str) -> Result<Vec<u32>, Error> {
        FakeModels.tokenize(card, room, text).await
    }

    async fn chat(
        &self,
        card: &ModelCard,
        room: Room,
        messages: &[Message],
    ) -> Result<String, Error> {
        FakeModels.chat(card, room, messages).await
    }
}
