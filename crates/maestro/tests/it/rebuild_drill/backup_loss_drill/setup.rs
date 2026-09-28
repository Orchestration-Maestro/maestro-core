//! Synthetic inputs and CLI commands that start the owned recovery drill.

use super::super::{fixture, ranking_oracle::QuestionInput, wipe_safety::QdrantOwner};
use crate::{
    knowledge_publish::{self, StubRouter},
    support::{Ended, Home, Running, synthetic},
};
use maestro_kernel::{artifact::Digest, gateway::Role, scope::LOCAL};
use maestro_knowledge::prepare::ChunkProfile;
use serde_json::Value;
use std::fs;
use ulid::Ulid;

pub(super) const COLLECTION: &str = "synthetic";

pub(super) struct Sources {
    pub(super) embedder: Digest,
    pub(super) reranker: Digest,
    pub(super) declaration: Digest,
    pub(super) corpus: Digest,
    pub(super) suite: Digest,
    pub(super) questions: Vec<QuestionInput>,
    pub(super) grants: Vec<String>,
}

pub(super) struct InitialPublication {
    pub(super) set_id: String,
    pub(super) job: Ulid,
    pub(super) generation: i64,
    pub(super) outcome: Value,
}

pub(super) fn load_sources(home: &Home) -> Sources {
    let embedder = knowledge_publish::t036_embedder_card(home);
    let reranker = knowledge_publish::card(home, Role::Reranker);
    let root = synthetic();
    let declaration = Digest::of(&fs::read(root.join("collection.json")).unwrap());
    let corpus = Digest::of(&fs::read(root.join("corpus/maestro-corpus.jsonl")).unwrap());
    let suite_bytes = fs::read(root.join("evals/synthetic.jsonl")).unwrap();
    let suite = Digest::of(&suite_bytes);
    let questions = fixture::suite_questions(&suite_bytes);
    let database = home.database();
    assert_eq!(
        database.put(&suite_bytes, "application/x-ndjson").unwrap(),
        suite
    );
    let grants = fixture::grant_names(&database.visible(LOCAL).unwrap());
    Sources {
        embedder,
        reranker,
        declaration,
        corpus,
        suite,
        questions,
        grants,
    }
}

pub(super) fn prepare_collection(
    home: &Home,
    owner: &QdrantOwner,
    router: &StubRouter,
    card: &str,
) {
    for (arguments, action) in [
        (
            &["--json", "knowledge", "import", "--collection", COLLECTION][..],
            "import",
        ),
        (
            &["--json", "knowledge", "quality", "--collection", COLLECTION][..],
            "quality",
        ),
        (
            &[
                "--json",
                "knowledge",
                "prepare",
                "--collection",
                COLLECTION,
                "--card",
                card,
            ][..],
            "prepare",
        ),
    ] {
        assert_success(action, &run_cli(home, arguments, &owner.url, router.url()));
    }
}

pub(super) fn publish_initial(
    home: &Home,
    owner: &QdrantOwner,
    router: &StubRouter,
    card: &str,
) -> InitialPublication {
    let database = home.database();
    let scopes = database.visible(LOCAL).unwrap();
    let set_id = database
        .latest_complete_chunk_set(
            &scopes,
            COLLECTION,
            ChunkProfile::default().chunker_version(),
        )
        .unwrap()
        .unwrap()
        .id;
    let result = run_cli(home, &publish_args(&set_id, card), &owner.url, router.url());
    assert_success("first publication", &result);
    let document = result.json();
    assert_eq!(document["state"], "succeeded", "{document}");
    let generation = document["current_generation"].as_i64().unwrap();
    assert_eq!(document["historical_generation"], generation);
    assert_eq!(document["projection_ready"], true);
    InitialPublication {
        set_id,
        job: Ulid::from_string(document["job"].as_str().unwrap()).unwrap(),
        generation,
        outcome: document["outcome"].clone(),
    }
}

pub(super) fn publish_args<'a>(set_id: &'a str, card: &'a str) -> [&'a str; 9] {
    [
        "--json",
        "knowledge",
        "publish",
        "--collection",
        COLLECTION,
        "--chunk-set",
        set_id,
        "--card",
        card,
    ]
}

pub(super) fn recovery_args<'a>(set_id: &'a str, card: &'a str) -> [&'a str; 10] {
    [
        "--json",
        "knowledge",
        "publish",
        "--collection",
        COLLECTION,
        "--chunk-set",
        set_id,
        "--card",
        card,
        "--again",
    ]
}

pub(super) fn run_cli(home: &Home, args: &[&str], qdrant: &str, router: &str) -> Ended {
    let mut command = home.command(args);
    command
        .env("MAESTRO_QDRANT_URL", qdrant)
        .env("MAESTRO_ROUTER_URL", router);
    Running::of(command).finish()
}

pub(super) fn assert_success(action: &str, result: &Ended) {
    assert_eq!(result.code, Some(0), "{action}: {result:?}");
    assert!(
        result.stderr.lines().all(|line| line.starts_with("job ")),
        "{action}: {result:?}"
    );
}
