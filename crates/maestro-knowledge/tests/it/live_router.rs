//! What the live tests share: the model router their variables name, and the
//! model card of its embedder, built from what the router and the model file
//! report, as T030 will record the real cards.
//!
//! `MAESTRO_ROUTER_URL` is where the router answers, such as
//! `http://127.0.0.1:8080`; `MAESTRO_ROUTER_EMBEDDER`, the router's entry for
//! the embedder, such as `embed`; and `MAESTRO_ROUTER_EMBEDDER_FILE`, the
//! model file the router serves under it. All three are read, and the file
//! hashed, before the first request.
#![cfg(test)]

use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{CardFields, Limits, ModelCard, Role, RouterEntry, Url},
};
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::{
    env,
    fmt::Write as _,
    fs::File,
    io::Read as _,
    num::{NonZeroU32, NonZeroUsize},
};
use tokio::runtime::Builder;

/// The value of `variable`, without which a live test cannot run.
pub(super) fn required(variable: &str) -> String {
    env::var(variable).unwrap_or_else(|_| panic!("set {variable}; see tests/it/live_router.rs"))
}

/// The router's address, and the card of its embedder recorded in `store`:
/// the build the embedder's `/props` reports, asked in free room, and the
/// SHA-256 of its model file, with BGE-M3's dimensions and context, the
/// native profile's model.
pub(super) fn embedder_card(store: &Store) -> (Url, ModelCard) {
    let base = Url::parse(&required("MAESTRO_ROUTER_URL")).unwrap();
    let entry = RouterEntry::parse(&required("MAESTRO_ROUTER_EMBEDDER")).unwrap();
    let model_file = file_digest(&required("MAESTRO_ROUTER_EMBEDDER_FILE"));
    // The first request, once every variable is read and the file hashed.
    let server_build = server_build(&base, &entry);
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
    (base, ModelCard::record(store, &fields).unwrap())
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
