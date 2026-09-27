//! `knowledge prepare` and `knowledge publish` refuse missing or unsuitable
//! model cards before they submit a job.

use super::support::{Home, Running};
use maestro_kernel::{
    artifact::{Digest, Store},
    chunk_set::NewChunkSet,
    gateway::{CardFields, Limits, ModelCard, Role, RouterEntry},
    generation::NewGeneration,
    job::NewJob,
    scope::Scope,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeSet, HashMap},
    io::{BufRead, BufReader, Read, Write},
    iter,
    net::{TcpListener, TcpStream},
    num::{NonZeroU32, NonZeroUsize},
    thread,
    time::{Duration, SystemTime},
};

/// A stored card of `role` in `home`, also recorded as a kernel artifact.
pub(super) fn card(home: &Home, role: Role) -> Digest {
    let (entry, dimensions) = match role {
        Role::Embedder => ("embed", Some(3)),
        Role::Reranker => ("rerank", None),
        Role::Answerer => ("answer", None),
    };
    let fields = CardFields {
        role,
        router_entry: RouterEntry::parse(entry).unwrap(),
        file_digest: Digest::of(b"model"),
        template_digest: None,
        server_build: "test-build".to_owned(),
        dimensions: dimensions.and_then(NonZeroUsize::new),
        limits: Limits {
            context_tokens: NonZeroU32::new(8192).unwrap(),
            output_tokens: None,
        },
        suite_results: Vec::new(),
    };
    let store = Store::new(home.data().join("artifacts"));
    let card = ModelCard::record(&store, &fields).unwrap();
    let bytes = store.get(card.digest()).unwrap();
    let recorded = home.database().put(&bytes, "application/json").unwrap();
    assert_eq!(&recorded, card.digest());
    recorded
}

/// T036's public synthetic embedder identity and native tokenizer canaries.
pub(super) fn t036_embedder_card(home: &Home) -> Digest {
    const IDENTITY: &str = "FakeModels synthetic protocol simulation/1";
    let fields = CardFields {
        role: Role::Embedder,
        router_entry: RouterEntry::parse("synthetic").unwrap(),
        file_digest: Digest::of(IDENTITY.as_bytes()),
        template_digest: None,
        server_build: "fake-models/1".to_owned(),
        dimensions: NonZeroUsize::new(32),
        limits: Limits {
            context_tokens: NonZeroU32::new(8192).unwrap(),
            output_tokens: None,
        },
        suite_results: Vec::new(),
    };
    let store = Store::new(home.data().join("artifacts"));
    let card = ModelCard::record(&store, &fields).unwrap();
    let bytes = store.get(card.digest()).unwrap();
    let recorded = home.database().put(&bytes, "application/json").unwrap();
    assert_eq!(&recorded, card.digest());
    recorded
}

/// A digest not present in this home's artifact store.
const MISSING: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// A loopback model router serving the committed tokenizer parity fixtures.
pub(super) struct StubRouter {
    url: String,
}

impl StubRouter {
    /// Serves tokenizer, embedding and reranking endpoints on a new loopback port.
    pub(super) fn serve() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let goldens = parity_goldens();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                reply(stream, &goldens);
            }
        });
        Self { url }
    }

    pub(super) fn url(&self) -> &str {
        &self.url
    }
}

/// Reads one router request and answers it with its pinned response.
fn reply(stream: TcpStream, goldens: &HashMap<String, Vec<u32>>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let path = line.split_whitespace().nth(1).unwrap().to_owned();
    let mut length = 0;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        let Some((name, value)) = header.trim_end().split_once(':') else {
            break;
        };
        if name.eq_ignore_ascii_case("content-length") {
            length = value.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    let answer = if path.ends_with("/props") {
        let build = if path.starts_with("/models/synthetic/") {
            "fake-models/1"
        } else {
            "test-build"
        };
        json!({"build_info": build})
    } else if path.ends_with("/tokenize") {
        let request: Value = serde_json::from_slice(&body).unwrap();
        let text = request["content"].as_str().unwrap();
        json!({"tokens": goldens.get(text).cloned().unwrap_or_else(|| fake_tokens(text))})
    } else if path.ends_with("/v1/embeddings") {
        let request: Value = serde_json::from_slice(&body).unwrap();
        let dimensions = if path.starts_with("/models/synthetic/") {
            32
        } else {
            3
        };
        let data = request["input"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, input)| {
                json!({
                    "index": index,
                    "embedding": fake_vector(input.as_str().unwrap(), dimensions)
                })
            })
            .collect::<Vec<_>>();
        json!({"data": data})
    } else if path.ends_with("/v1/rerank") {
        let request: Value = serde_json::from_slice(&body).unwrap();
        let query = request["query"].as_str().unwrap();
        let results = request["documents"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, document)| {
                json!({
                    "index": index,
                    "relevance_score": fake_rerank(query, document.as_str().unwrap())
                })
            })
            .collect::<Vec<_>>();
        json!({"results": results})
    } else {
        panic!("the test router has no route for {path}");
    }
    .to_string();
    let mut stream = reader.into_inner();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{answer}",
        answer.len()
    )
    .unwrap();
}

/// `FakeModels`' deterministic synthetic embedding used by the router stub.
fn fake_vector(text: &str, dimensions: usize) -> Vec<f32> {
    use sha2::{Digest as _, Sha256};

    (0_u32..)
        .flat_map(|block| {
            let digest = Sha256::new()
                .chain_update(text)
                .chain_update(block.to_be_bytes())
                .finalize();
            let (low, high) = (digest.iter().step_by(2), digest.iter().skip(1).step_by(2));
            low.zip(high)
                .map(|(low, high)| f32::from(i16::from_le_bytes([*low, *high])) / 32768.0)
                .collect::<Vec<_>>()
        })
        .take(dimensions)
        .collect()
}

/// `FakeModels`' tokens used after the native-parity canaries pass.
fn fake_tokens(text: &str) -> Vec<u32> {
    use sha2::{Digest as _, Sha256};

    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| {
            let [first, second, third, fourth, ..]: [u8; 32] = Sha256::digest(word).into();
            u32::from_le_bytes([first, second, third, fourth])
        })
        .collect()
}

/// `FakeModels`' word-overlap rerank score.
fn fake_rerank(query: &str, document: &str) -> f64 {
    let words = |text: &str| {
        text.split(|character: char| !character.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_lowercase)
            .collect::<BTreeSet<_>>()
    };
    let wanted = words(query);
    let held = words(document);
    f64::from(u32::try_from(wanted.intersection(&held).count()).unwrap_or(u32::MAX))
}

/// The router qualification fixtures and their expected ordered token IDs.
fn parity_goldens() -> HashMap<String, Vec<u32>> {
    const PARITY: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maestro-knowledge/src/prepare/native-parity.json"
    ));
    let file: Value = serde_json::from_str(PARITY).unwrap();
    file["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|fixture| (input(&fixture["input"]), ids(&fixture["ids"])))
        .collect()
}

/// Expands the input parts of one tokenizer parity fixture.
fn input(parts: &Value) -> String {
    let mut input = String::new();
    for part in parts.as_array().unwrap() {
        if let Some(text) = part.as_str() {
            input.push_str(text);
        } else {
            let repeated = part["repeat"].as_str().unwrap();
            for _ in 0..part["times"].as_u64().unwrap() {
                input.push_str(repeated);
            }
        }
    }
    input
}

/// Expands the expected token-ID runs of one parity fixture.
fn ids(parts: &Value) -> Vec<u32> {
    let mut ids = Vec::new();
    for part in parts.as_array().unwrap() {
        if let Some(id) = part.as_u64() {
            ids.push(u32::try_from(id).unwrap());
        } else {
            let repeated = u32::try_from(part["repeat"].as_u64().unwrap()).unwrap();
            ids.extend(iter::repeat_n(
                repeated,
                usize::try_from(part["times"].as_u64().unwrap()).unwrap(),
            ));
        }
    }
    ids
}

#[test]
fn prepare_refuses_missing_and_non_embedder_cards_before_submitting_a_job() {
    let home = Home::new();
    home.add_synthetic();
    for (digest, expected) in [
        (MISSING.to_owned(), "recorded"),
        (card(&home, Role::Reranker).as_str().to_owned(), "embedder"),
    ] {
        let result = home.run(&[
            "knowledge",
            "prepare",
            "--collection",
            "synthetic",
            "--card",
            &digest,
            "--json",
        ]);
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(result.stdout.is_empty(), "no job was submitted: {result:?}");
        assert!(result.stderr.contains(expected), "{result:?}");
    }
}

#[test]
fn publish_refuses_missing_and_non_embedder_cards_before_submitting_a_job() {
    let home = Home::new();
    home.add_synthetic();
    for (digest, expected) in [
        (MISSING.to_owned(), "recorded"),
        (card(&home, Role::Reranker).as_str().to_owned(), "embedder"),
    ] {
        let result = home.run(&[
            "knowledge",
            "publish",
            "--collection",
            "synthetic",
            "--card",
            &digest,
            "--json",
        ]);
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(result.stdout.is_empty(), "no job was submitted: {result:?}");
        assert!(result.stderr.contains(expected), "{result:?}");
    }
}

#[test]
fn verify_refuses_a_collection_without_a_published_generation() {
    let home = Home::new();
    home.add_synthetic();
    let result = home.run(&["knowledge", "verify", "--collection", "synthetic", "--json"]);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(result.stdout.is_empty(), "no job was submitted: {result:?}");
    assert!(
        result.stderr.contains("no published generation"),
        "{result:?}"
    );
}

#[test]
fn verify_reports_a_qdrant_failure_as_a_failed_job() {
    let home = Home::new();
    home.add_synthetic();
    let database = home.database();
    let manifest = database.put(b"{}", "application/json").unwrap();
    let set = NewChunkSet {
        id: "verify-set",
        collection_id: "synthetic",
        chunk_profile: "test-profile",
        counter_contract_id: "test-counter",
    };
    database.begin_chunk_set(&set).unwrap();
    database.complete_chunk_set(set.id, &manifest).unwrap();
    let generation = database
        .create_generation(&NewGeneration {
            collection_id: "synthetic".to_owned(),
            chunk_set_id: set.id.to_owned(),
            embedding_profile: "test-embedder".to_owned(),
            sparse_profile: "test-sparse".to_owned(),
        })
        .unwrap();
    database.verify_generation(generation.id, 0).unwrap();
    database.publish_generation(generation.id).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || drop(listener.accept().unwrap().0));

    let mut command = home.command(&["knowledge", "verify", "--collection", "synthetic", "--json"]);
    command.env("MAESTRO_QDRANT_URL", url);
    let result = Running::of(command).finish();
    assert_eq!(result.code, Some(1), "{result:?}");
    assert!(result.stderr.contains("job "), "{result:?}");
    let document: Value = serde_json::from_str(&result.stdout).unwrap();
    assert_eq!(document["schema"], "maestro-cli/knowledge-verify/1");
    assert_eq!(document["state"], "failed");
    assert!(
        document["outcome"]["error"]
            .as_str()
            .unwrap()
            .contains("Qdrant")
    );
}

#[test]
fn publish_refuses_a_live_lease_and_names_the_holding_job() {
    let home = Home::new();
    home.add_synthetic();
    let card = card(&home, Role::Embedder);
    let database = home.database();
    let manifest = database.put(b"{}", "application/json").unwrap();
    let new_set = NewChunkSet {
        id: "complete-set",
        collection_id: "synthetic",
        chunk_profile: "test-profile",
        counter_contract_id: "test-counter",
    };
    database.begin_chunk_set(&new_set).unwrap();
    database.complete_chunk_set(new_set.id, &manifest).unwrap();
    let scope: Scope = "workspace/default/collection/synthetic".parse().unwrap();
    let inputs = json!({"blocking": true});
    let new = NewJob {
        kind: "knowledge.publish",
        inputs: &inputs,
        scope: &scope,
        resource: Some("collection/synthetic/publish"),
    };
    let at = SystemTime::now();
    let job = database.submit_job(&new, at).unwrap();
    let _lease = database
        .take_job(job.id, "test-holder", at, Duration::from_secs(60))
        .unwrap();

    let mut command = home.command(&[
        "knowledge",
        "publish",
        "--collection",
        "synthetic",
        "--card",
        card.as_str(),
        "--chunk-set",
        new_set.id,
        "--json",
    ]);
    command.env_remove("MAESTRO_ROUTER_URL");
    command.env_remove("MAESTRO_QDRANT_URL");
    let result = Running::of(command).finish();
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(result.stdout.is_empty(), "{result:?}");
    assert!(result.stderr.contains(&job.id.to_string()), "{result:?}");
}

#[test]
fn prepare_run_twice_finds_the_same_succeeded_job() {
    let home = Home::new();
    home.add_synthetic();
    let card = card(&home, Role::Embedder);
    let router = StubRouter::serve();
    let mut results = Vec::new();
    for _ in 0..2 {
        let mut command = home.command(&[
            "knowledge",
            "prepare",
            "--collection",
            "synthetic",
            "--card",
            card.as_str(),
            "--json",
        ]);
        command.env("MAESTRO_ROUTER_URL", router.url());
        results.push(Running::of(command).finish());
    }
    let documents: Vec<Value> = results
        .iter()
        .map(|result| {
            assert_eq!(result.code, Some(0), "{result:?}");
            assert!(result.stderr.contains("job "), "{result:?}");
            assert_eq!(result.stdout.lines().count(), 1, "{result:?}");
            serde_json::from_str(&result.stdout).unwrap()
        })
        .collect();
    assert_eq!(documents[0]["schema"], "maestro-cli/knowledge-prepare/1");
    assert_eq!(documents[0]["state"], "succeeded");
    assert_eq!(documents[0]["job"], documents[1]["job"]);
}
