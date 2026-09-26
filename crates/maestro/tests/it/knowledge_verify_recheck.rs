//! `knowledge verify` must read its artifacts again on every invocation.

use super::support::{Ended, Home, Running};
use maestro_kernel::{
    artifact::Digest,
    chunk_set::{Chunk, NewChunkSet},
    document::{Collection, Document, Revision, RevisionStatus, Source},
    evidence::Span,
    generation::NewGeneration,
    job::NewJob,
    scope::Scope,
};
use reqwest::{Client, Method, StatusCode};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env, fs, process,
    time::{Duration, SystemTime},
};
use tokio::runtime::Builder;

/// The scratch Qdrant's HTTP API, never the real kernel's ports.
const SCRATCH_QDRANT_HTTP: &str = "http://127.0.0.1:16733";
/// The scratch Qdrant's gRPC API, never the real kernel's ports.
const SCRATCH_QDRANT_GRPC: &str = "http://127.0.0.1:16734";

#[test]
fn verify_checks_artifacts_again_on_every_run() {
    if env::var("MAESTRO_QDRANT_URL").as_deref() != Ok(SCRATCH_QDRANT_GRPC) {
        eprintln!(
            "skipped: MAESTRO_QDRANT_URL must name the scratch Qdrant at {SCRATCH_QDRANT_GRPC}"
        );
        return;
    }

    let home = Home::new();
    let collection_id = format!("verify-recheck-{}", process::id());
    let (generation_id, prepared) = published_generation(&home, &collection_id);
    let generation = generation_id.to_string();
    let qdrant_collection = ["maestro-", &collection_id, "-g", &generation].concat();
    let qdrant_alias = ["maestro-", &collection_id].concat();
    let path = create_qdrant_generation(&qdrant_collection, &qdrant_alias);

    let first = run_verify(&home, &collection_id);
    let digest = prepared.as_str();
    let (prefix, rest) = digest.split_at_checked(2).unwrap_or_default();
    let (middle, _) = rest.split_at_checked(2).unwrap_or_default();
    let artifact = home
        .data()
        .join("artifacts")
        .join("sha256")
        .join(prefix)
        .join(middle)
        .join(digest);
    fs::write(artifact, b"corrupted input").unwrap();
    let second = run_verify(&home, &collection_id);
    let _ = qdrant_http(Method::DELETE, &path, None);

    assert_eq!(first.code, Some(0), "{first:?}");
    assert_eq!(second.code, Some(1), "{second:?}");
    let first = first.json();
    let second = second.json();
    assert_ne!(first["job"], second["job"]);
    assert!(
        second["outcome"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding.as_str().unwrap().contains("digest mismatch"))
    );
}

#[test]
fn verify_refuses_a_live_check_using_the_same_resource() {
    let home = Home::new();
    let collection_id = format!("verify-lease-{}", process::id());
    let (generation, _) = published_generation(&home, &collection_id);
    let database = home.database();
    let scope: Scope = format!("workspace/default/collection/{collection_id}")
        .parse()
        .unwrap();
    let inputs = json!({"again": "holder", "collection": collection_id, "generation": generation});
    let resource = format!("collection/{collection_id}/verify");
    let new = NewJob {
        kind: "knowledge.verify",
        inputs: &inputs,
        scope: &scope,
        resource: Some(&resource),
    };
    let now = SystemTime::now();
    let held = database.submit_job(&new, now).unwrap();
    let _lease = database
        .take_job(held.id, "verify-holder", now, Duration::from_secs(60))
        .unwrap();

    let result = run_verify(&home, &collection_id);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(result.stdout.is_empty(), "{result:?}");
    assert!(result.stderr.contains(&held.id.to_string()), "{result:?}");
}

/// Records one published generation with one prepared artifact.
fn published_generation(home: &Home, collection_id: &str) -> (i64, Digest) {
    let database = home.database();
    database
        .record_collection(&Collection {
            id: collection_id.to_owned(),
            title: "Verification recheck".to_owned(),
            visibility: "private".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    database
        .record_source(&Source {
            collection_id: collection_id.to_owned(),
            id: "source".to_owned(),
            kind: "import".to_owned(),
            transport: None,
            reference: "corpus_root:source.jsonl".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    database
        .record_document(&Document {
            id: "document".to_owned(),
            collection_id: collection_id.to_owned(),
            source_id: "source".to_owned(),
            source_ref: "https://example.org/document".to_owned(),
        })
        .unwrap();
    let original = database.put(b"original", "text/markdown").unwrap();
    let canonical = database.put(b"canonical", "application/json").unwrap();
    let prepared = database.put(b"prepared input", "text/plain").unwrap();
    let revision_id = "revision";
    database
        .record_revision(&Revision {
            id: revision_id.to_owned(),
            document_id: "document".to_owned(),
            original_digest: original,
            canonical_digest: canonical,
            status: RevisionStatus::Valid,
            captured_at: None,
            metadata: serde_json::Map::new(),
        })
        .unwrap();
    let chunk_set_id = "chunk-set";
    database
        .begin_chunk_set(&NewChunkSet {
            id: chunk_set_id,
            collection_id,
            chunk_profile: "test-profile",
            counter_contract_id: "test-counter",
        })
        .unwrap();
    database
        .record_chunks(
            chunk_set_id,
            revision_id,
            &[Chunk {
                id: "chunk".to_owned(),
                revision_id: revision_id.to_owned(),
                section_id: None,
                digest: prepared.clone(),
                token_count: 1,
                span: Span { start: 0, end: 1 },
            }],
        )
        .unwrap();
    let manifest = database.put(b"{}", "application/json").unwrap();
    database
        .complete_chunk_set(chunk_set_id, &manifest)
        .unwrap();
    let generation = database
        .create_generation(&NewGeneration {
            collection_id: collection_id.to_owned(),
            chunk_set_id: chunk_set_id.to_owned(),
            embedding_profile: "test-embedder".to_owned(),
            sparse_profile: "test-sparse".to_owned(),
        })
        .unwrap();
    database.verify_generation(generation.id, 1).unwrap();
    database.publish_generation(generation.id).unwrap();
    (generation.id, prepared)
}

/// Creates a Qdrant collection with one point and its expected alias.
fn create_qdrant_generation(collection: &str, alias: &str) -> String {
    let path = format!("/collections/{collection}");
    let _ = qdrant_http(Method::DELETE, &path, None);
    assert!(
        qdrant_http(
            Method::PUT,
            &path,
            Some(json!({"vectors": {"size": 1, "distance": "Cosine"}})),
        )
        .is_success()
    );
    assert!(
        qdrant_http(
            Method::PUT,
            &format!("{path}/points?wait=true"),
            Some(json!({
                "points": [{
                    "id": "00000000-0000-0000-0000-000000000001",
                    "vector": [0.1],
                }],
            })),
        )
        .is_success()
    );
    assert!(
        qdrant_http(
            Method::POST,
            "/collections/aliases",
            Some(json!({
                "actions": [{
                    "create_alias": {
                        "collection_name": collection,
                        "alias_name": alias,
                    },
                }],
            })),
        )
        .is_success()
    );
    path
}

/// Runs `knowledge verify` against the required scratch Qdrant.
fn run_verify(home: &Home, collection_id: &str) -> Ended {
    let mut command = home.command(&[
        "knowledge",
        "verify",
        "--collection",
        collection_id,
        "--json",
    ]);
    command.env("MAESTRO_QDRANT_URL", SCRATCH_QDRANT_GRPC);
    Running::of(command).finish()
}

/// Sends a request to the required scratch Qdrant's HTTP API.
fn qdrant_http(method: Method, path: &str, body: Option<Value>) -> StatusCode {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move {
            let client = Client::new();
            let mut request = client.request(method, format!("{SCRATCH_QDRANT_HTTP}{path}"));
            if let Some(body) = body {
                request = request.json(&body);
            }
            request.send().await.unwrap().status()
        })
}
