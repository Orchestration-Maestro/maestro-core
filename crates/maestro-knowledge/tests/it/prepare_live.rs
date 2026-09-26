//! `knowledge prepare` on this machine's kernel, live: an explicit local
//! run, never a pass when ignored. It prepares a collection the real import
//! filled and the quality gate decided (T020), and prints what T023's step 3
//! records: the chunk count, the wall time and the router calls it took.
//!
//! It reads `MAESTRO_PREPARE_COLLECTION`, the collection, such as `ctm`, and
//! the router's variables that `live_router.rs` names. It opens the kernel
//! as the command line does: in the directories the environment names,
//! reading as the local principal once `config.toml` is applied. It records
//! the embedder's model card in the kernel's artifact store, and runs the
//! preparation as a leased job: of kind `knowledge.prepare`, in the
//! collection's scope, holding the resource `collection/<id>/prepare`, with
//! the chunk set it builds as its frozen input, and a step journaled after
//! each batch. A rerun finds the job succeeded and prints its outcome.
//!
//! Every request asks for free room: run it against a router with free room
//! (T002) only.
#![cfg(test)]

use super::live_router::{embedder_card, required};
use maestro_kernel::{
    artifact::Store,
    gateway::{Error, Message, ModelCard, ModelPort, Room, RouterClient},
    job::{JobState, NewJob},
    paths::{self, Environment},
    scope::{Config, LOCAL, Scope},
    store::Database,
};
use maestro_knowledge::prepare::{self, Report, RouterTokenizer};
use serde_json::json;
use std::{
    ops::ControlFlow,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

/// How long the job's lease lasts between two steps: the deduplication
/// reads every eligible revision before the first step.
const TERM: Duration = Duration::from_secs(1800);

/// The router client, counting the texts it tokenizes.
#[derive(Debug)]
struct Counted {
    /// The client.
    client: RouterClient,
    /// The texts tokenized so far.
    tokenized: Arc<AtomicU64>,
}

impl ModelPort for Counted {
    async fn embed(
        &self,
        card: &ModelCard,
        room: Room,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, Error> {
        self.client.embed(card, room, inputs).await
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
        self.tokenized.fetch_add(1, Ordering::Relaxed);
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

#[test]
#[ignore = "prepares a collection of this machine's kernel through a model router with free room; \
            run explicitly"]
fn the_collection_prepares_as_a_leased_job() {
    let collection = required("MAESTRO_PREPARE_COLLECTION");
    let environment = Environment::current();
    let data = paths::data_dir(&environment).unwrap();
    let config = Config::load(&paths::config_dir(&environment).unwrap()).unwrap();
    let database = Database::open_in(&data).unwrap();
    database.apply_config(&config).unwrap();
    let scopes = database.visible(LOCAL).unwrap();
    let (base, card) = embedder_card(&Store::new(data.join("artifacts")));
    println!("embedder card: sha256:{}", card.digest().as_str());
    let tokenized = Arc::new(AtomicU64::new(0));
    let port = Counted {
        client: RouterClient::new(base).unwrap(),
        tokenized: Arc::clone(&tokenized),
    };
    let tokenizer = RouterTokenizer::qualify(port, card).unwrap();
    let chunk_set = prepare::chunk_set_id(&database, &scopes, &collection, &tokenizer).unwrap();
    let scope: Scope = format!("workspace/default/collection/{collection}")
        .parse()
        .unwrap();
    let job = database
        .submit_job(
            &NewJob {
                kind: "knowledge.prepare",
                inputs: &json!({ "collection": collection, "chunk_set": chunk_set }),
                scope: &scope,
                resource: Some(&format!("collection/{collection}/prepare")),
            },
            SystemTime::now(),
        )
        .unwrap();
    println!("job {} for the chunk set {chunk_set}", job.id);
    if job.state == JobState::Succeeded {
        println!("succeeded before: {}", job.outcome.unwrap());
        return;
    }
    let mut lease = database
        .take_job(job.id, "prepare-live", SystemTime::now(), TERM)
        .unwrap();
    let (started, before) = (Instant::now(), tokenized.load(Ordering::Relaxed));
    let mut first_step = None;
    let report =
        prepare::prepare_observed(&database, &scopes, &collection, &tokenizer, &mut |report| {
            first_step.get_or_insert_with(|| started.elapsed());
            let step = json!({
                "prepared": report.prepared,
                "chunks": report.chunks,
                "refused": report.refused,
            });
            match database.progress(&mut lease, SystemTime::now(), TERM, &step) {
                Ok(_) => ControlFlow::Continue(()),
                Err(_) => ControlFlow::Break(()),
            }
        });
    let elapsed = started.elapsed();
    let calls = tokenized.load(Ordering::Relaxed) - before;
    let report: Report = report.unwrap();
    database
        .complete_job(
            &lease,
            JobState::Succeeded,
            &serde_json::to_value(&report).unwrap(),
        )
        .unwrap();
    println!(
        "prepared in {elapsed:.1?}, the first batch after {first_step:.1?}: {calls} texts \
         tokenized"
    );
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
