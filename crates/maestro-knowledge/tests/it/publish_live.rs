//! A chunk set of this machine's kernel published into Qdrant, live: an
//! explicit local run, never a pass when ignored. It publishes a complete
//! chunk set (T023) through the router's embedder in free room, into the
//! Qdrant `MAESTRO_QDRANT_URL` names, and prints what T026 measures for T028
//! and T030: the embed rate and the upsert rate.
//!
//! It reads `MAESTRO_PUBLISH_CHUNK_SET`, the chunk set, `MAESTRO_QDRANT_URL`,
//! where Qdrant's gRPC API answers, such as `http://127.0.0.1:6334`, and
//! `MAESTRO_ROUTER_URL`, where the router answers. It opens the kernel as
//! the command line does: in the directories the environment names, reading
//! as the local principal once `config.toml` is applied; point
//! `XDG_DATA_HOME` at a copy of the kernel to leave the real one alone. The
//! embedder is the one whose card the chunk set's counter names
//! (`router/1:sha256:<the card's digest>`), loaded from the kernel's
//! artifact store. The publication runs as a leased job, of kind
//! `knowledge.publish`, in the collection's scope, holding the resource
//! `collection/<id>/publish`, with the chunk set as its frozen input and a
//! step journaled after each batch.
//!
//! Every request asks for free room: the router refuses rather than unload a
//! model, and a refusal stops the run with the generation building, which a
//! rerun resumes.
#![cfg(test)]

use super::live_router::required;
use maestro_kernel::{
    artifact::{Digest, Store},
    chunk_set::ChunkSet,
    gateway::{Error, Message, ModelCard, ModelPort, Room, RouterClient, Url},
    job::{JobState, Lease, NewJob},
    paths::{self, Environment},
    scope::{Config, LOCAL, Scope, ScopeSet},
    store::Database,
};
use maestro_knowledge::index::{Progress, Projection, Qdrant};
use serde_json::json;
use std::{
    ops::ControlFlow,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};

/// How long the job's lease lasts between two steps: the first pass reads
/// every chunk's prepared input before the first step.
const TERM: Duration = Duration::from_secs(1800);

/// One embedding call: its inputs, when it started and when it answered.
type Call = (usize, Instant, Instant);

/// The router client, timing each embedding call.
#[derive(Debug)]
struct Timed {
    /// The client.
    client: RouterClient,
    /// The embedding calls so far.
    calls: Arc<Mutex<Vec<Call>>>,
}

impl ModelPort for Timed {
    async fn embed(
        &self,
        card: &ModelCard,
        room: Room,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, Error> {
        let started = Instant::now();
        let vectors = self.client.embed(card, room, inputs).await;
        let call = (inputs.len(), started, Instant::now());
        self.calls.lock().unwrap().push(call);
        vectors
    }

    async fn rerank(
        &self,
        card: &ModelCard,
        room: Room,
        query: &str,
        documents: &[String],
    ) -> Result<Vec<f64>, Error> {
        self.client.rerank(card, room, query, documents).await
    }

    async fn tokenize(&self, card: &ModelCard, room: Room, text: &str) -> Result<Vec<u32>, Error> {
        self.client.tokenize(card, room, text).await
    }

    async fn chat(
        &self,
        card: &ModelCard,
        room: Room,
        messages: &[Message],
    ) -> Result<String, Error> {
        self.client.chat(card, room, messages).await
    }
}

/// `count` over `seconds`, per second.
fn rate(count: usize, seconds: f64) -> f64 {
    f64::from(u32::try_from(count).unwrap()) / seconds
}

/// The kernel as the command line opens it, in the directories the
/// environment names, and what the local principal reads once `config.toml`
/// is applied, with the kernel's data directory.
fn kernel() -> (Database, ScopeSet, PathBuf) {
    let environment = Environment::current();
    let data = paths::data_dir(&environment).unwrap();
    let config = Config::load(&paths::config_dir(&environment).unwrap()).unwrap();
    let database = Database::open_in(&data).unwrap();
    database.apply_config(&config).unwrap();
    let scopes = database.visible(LOCAL).unwrap();
    (database, scopes, data)
}

/// The embedder's card the counter of `set` names, from the artifact store
/// of the kernel in `data`.
fn card_of(set: &ChunkSet, data: &Path) -> ModelCard {
    let digest = set
        .counter_contract_id
        .strip_prefix("router/1:sha256:")
        .unwrap();
    let store = Store::new(data.join("artifacts"));
    ModelCard::load(&store, &Digest::parse(digest).unwrap()).unwrap()
}

/// The lease of the job publishing `set`, taken for this run, and the last
/// step it journaled; none when the job succeeded before.
fn lease(
    database: &Database,
    scopes: &ScopeSet,
    set: &ChunkSet,
) -> Option<(Lease, Option<Progress>)> {
    let scope: Scope = format!("workspace/default/collection/{}", set.collection_id)
        .parse()
        .unwrap();
    let inputs = json!({ "chunk_set": set.id });
    let resource = format!("collection/{}/publish", set.collection_id);
    let new = NewJob {
        kind: "knowledge.publish",
        inputs: &inputs,
        scope: &scope,
        resource: Some(&resource),
    };
    let job = database.submit_job(&new, SystemTime::now()).unwrap();
    if job.state == JobState::Succeeded {
        println!("succeeded before: {}", job.outcome.unwrap());
        return None;
    }
    let resume = database
        .last_progress(scopes, job.id)
        .unwrap()
        .map(|step| serde_json::from_value::<Progress>(step.data).unwrap());
    let lease = database
        .take_job(job.id, "publish-live", SystemTime::now(), TERM)
        .unwrap();
    Some((lease, resume))
}

/// Prints the rates of a run that started at `started` and took `elapsed`:
/// its embedding `calls`, each followed by the step at the same place of
/// `steps` once its points were written, of a chunk set of `chunks` chunks
/// and `tokens` tokens.
fn rates(
    started: Instant,
    elapsed: Duration,
    calls: &[Call],
    steps: &[Instant],
    (chunks, tokens): (usize, u64),
) {
    let embedded: usize = calls.iter().map(|(inputs, _, _)| inputs).sum();
    let embedding: f64 = calls
        .iter()
        .map(|(_, from, to)| (*to - *from).as_secs_f64())
        .sum();
    let writing: f64 = calls
        .iter()
        .zip(steps)
        .map(|((_, _, answered), step)| (*step - *answered).as_secs_f64())
        .sum();
    let first = calls.first().map(|(_, from, _)| *from - started);
    let last = steps.last().map(|step| started + elapsed - *step);
    println!("first embedding call after {first:.1?} (the first pass); check and alias {last:.1?}");
    println!(
        "embedded {embedded} chunks in {} calls, {embedding:.1} s: {:.0} chunks/s",
        calls.len(),
        rate(embedded, embedding)
    );
    println!(
        "built and wrote their points in {writing:.1} s: {:.0} points/s",
        rate(embedded, writing)
    );
    if embedded == chunks {
        let tokens = f64::from(u32::try_from(tokens).unwrap());
        println!("{:.0} tokens/s embedded", tokens / embedding);
    }
}

#[tokio::test]
#[ignore = "publishes a chunk set of this machine's kernel into a Qdrant through a model router \
            with free room; run explicitly"]
async fn a_chunk_set_publishes_as_a_leased_job() {
    let chunk_set = required("MAESTRO_PUBLISH_CHUNK_SET");
    let qdrant = Qdrant::new(&required("MAESTRO_QDRANT_URL")).unwrap();
    let router = Url::parse(&required("MAESTRO_ROUTER_URL")).unwrap();
    let (database, scopes, data) = kernel();
    let set = database.chunk_set(&scopes, &chunk_set).unwrap().unwrap();
    let card = card_of(&set, &data);
    let chunks = database.chunks(&scopes, &chunk_set).unwrap();
    let tokens: u64 = chunks.iter().map(|chunk| chunk.token_count).sum();
    println!("{chunk_set}: {} chunks, {tokens} tokens", chunks.len());
    let Some((mut lease, resume)) = lease(&database, &scopes, &set) else {
        return;
    };
    let calls = Arc::default();
    let port = Timed {
        client: RouterClient::new(router).unwrap(),
        calls: Arc::clone(&calls),
    };
    let projection = Projection {
        database: &database,
        scopes: &scopes,
        qdrant: &qdrant,
        port: &port,
        card: &card,
    };
    let started = Instant::now();
    let mut steps = Vec::new();
    let report = projection
        .publish_observed(&chunk_set, resume.as_ref(), &mut |progress: &Progress| {
            steps.push(Instant::now());
            let step = serde_json::to_value(progress).unwrap();
            match database.progress(&mut lease, SystemTime::now(), TERM, &step) {
                Ok(_) => ControlFlow::Continue(()),
                Err(_) => ControlFlow::Break(()),
            }
        })
        .await;
    let elapsed = started.elapsed();
    let report = report.unwrap();
    let outcome = serde_json::to_value(&report).unwrap();
    database
        .complete_job(&lease, JobState::Succeeded, &outcome)
        .unwrap();
    println!(
        "published in {elapsed:.1?}: {}",
        serde_json::to_string_pretty(&report).unwrap()
    );
    let calls = calls.lock().unwrap().clone();
    rates(started, elapsed, &calls, &steps, (chunks.len(), tokens));
}
