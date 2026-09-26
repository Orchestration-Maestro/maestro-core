//! The router tokenizer's parity with the native counter, live: an explicit
//! local test, like the native ones, never a pass when ignored. It qualifies
//! the model router's embedder on the 41 parity fixtures, whose goldens `just
//! native` checks against the native counter, so `just parity` runs both.
//!
//! It reads `MAESTRO_ROUTER_URL`, where the router answers, such as
//! `http://127.0.0.1:8080`; `MAESTRO_ROUTER_EMBEDDER`, the router's entry for
//! the embedder, such as `embed`; and `MAESTRO_ROUTER_EMBEDDER_FILE`, the
//! model file the router serves under it. It reads all three, and hashes the
//! file, before its first request. The card it builds records that file's
//! SHA-256 and the build the embedder's `/props` reports, as T030 will record
//! the real cards.
//!
//! Every request asks for free room, which only a router with free room
//! (T002, redeployed) honours: an older one ignores it, and may unload the
//! chat model to load the embedder. Run it against no other.
#![cfg(test)]

use maestro_canonicalization::TokenCounter;
use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{CardFields, Limits, ModelCard, Role, RouterClient, RouterEntry, Url},
};
use maestro_knowledge::prepare::RouterTokenizer;
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::{
    env,
    fmt::Write as _,
    fs::{self, File},
    io::Read as _,
    num::{NonZeroU32, NonZeroUsize},
    process,
};
use tokio::runtime::Builder;

/// The value of `variable`, without which the test cannot run.
fn required(variable: &str) -> String {
    env::var(variable).unwrap_or_else(|_| panic!("set {variable}; see tests/it/router_parity.rs"))
}

/// The llama.cpp build the embedder under `entry` reports through `/props`,
/// asked in free room.
fn server_build(base: &Url, entry: &RouterEntry) -> String {
    let url = base
        .join(&format!("models/{}/props", entry.as_str()))
        .unwrap();
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let props: Value = runtime.block_on(async {
        let client = Client::builder().no_proxy().build().unwrap();
        let answer = client
            .get(url)
            .header("X-Model-Router-Room", "free")
            .send()
            .await
            .unwrap();
        answer.error_for_status().unwrap().json().await.unwrap()
    });
    props["build_info"].as_str().unwrap().to_owned()
}

/// The SHA-256 of the file at `path`, read a block at a time.
fn file_digest(path: &str) -> Digest {
    let mut file = File::open(path).unwrap();
    let (mut hasher, mut block) = (Sha256::new(), vec![0; 1 << 20]);
    loop {
        let read = file.read(&mut block).unwrap();
        if read == 0 {
            break;
        }
        hasher.update(&block[..read]);
    }
    let hex = hasher
        .finalize()
        .iter()
        .fold(String::new(), |mut hex, byte| {
            write!(hex, "{byte:02x}").unwrap();
            hex
        });
    Digest::parse(&hex).unwrap()
}

#[test]
#[ignore = "needs a model router with free room (T002 redeployed): an older one may unload the \
            chat model; run explicitly"]
fn the_router_gives_every_parity_fixture_the_native_ids() {
    let base = Url::parse(&required("MAESTRO_ROUTER_URL")).unwrap();
    let entry = RouterEntry::parse(&required("MAESTRO_ROUTER_EMBEDDER")).unwrap();
    let model_file = file_digest(&required("MAESTRO_ROUTER_EMBEDDER_FILE"));
    // The first request, once every variable is read and the file hashed.
    let server_build = server_build(&base, &entry);
    // BGE-M3's dimensions and context: only the native profile's model can
    // give the goldens.
    let fields = CardFields {
        role: Role::Embedder,
        server_build,
        router_entry: entry,
        file_digest: model_file,
        template_digest: None,
        dimensions: NonZeroUsize::new(1024),
        limits: Limits {
            context_tokens: NonZeroU32::new(8192).unwrap(),
            output_tokens: None,
        },
        suite_results: Vec::new(),
    };
    let scratch = env::temp_dir().join(format!("maestro-router-parity-{}", process::id()));
    let card = ModelCard::record(&Store::new(&scratch), &fields).unwrap();
    fs::remove_dir_all(&scratch).unwrap();
    let tokenizer = RouterTokenizer::qualify(RouterClient::new(base).unwrap(), card).unwrap();
    tokenizer.verify().unwrap();
    assert_eq!(
        tokenizer.token_ids("Hello world").unwrap(),
        [0, 35378, 8999, 2]
    );
    println!(
        "router parity: {} gives the 41 fixtures the native IDs",
        tokenizer.contract_id()
    );
}
