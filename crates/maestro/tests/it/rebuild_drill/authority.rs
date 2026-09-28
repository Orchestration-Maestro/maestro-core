//! Backup manifest, restored authority, retained work and projection assertions.

use super::{fixture::grant_names, qdrant::QdrantScratch, ranking_oracle::Snapshot};
use crate::{backup_restore, support::Home};
use maestro_kernel::{
    artifact::{Digest, Store},
    chunk_set::ChunkSetState,
    generation::GenerationState,
    job::JobState,
    retrieval::IDENTIFIER_PROFILE,
    scope::LOCAL,
};
use maestro_knowledge::lexical;
use serde_json::{Value, json};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Component, Path},
};
use ulid::Ulid;

const COLLECTION: &str = "synthetic";
const ALIAS: &str = "maestro-synthetic";
const SCOPE_TAGS: [&str; 3] = [
    "workspace/default",
    "workspace/default/collection/synthetic",
    "workspace/default/collection/synthetic/source/handbook",
];

pub(super) fn source_evidence(
    home: &Home,
    job: Ulid,
    generation: i64,
    card: &str,
    outcome: &Value,
) -> (Value, Value) {
    let database = home.database();
    let scopes = database.visible(LOCAL).unwrap();
    let job_record = database.job(&scopes, job).unwrap().unwrap();
    assert_eq!(job_record.state, JobState::Succeeded);
    assert_eq!(job_record.outcome.as_ref(), Some(outcome));
    let progress = database.last_progress(&scopes, job).unwrap().unwrap().data;
    let current = database.generation(&scopes, generation).unwrap().unwrap();
    let authority = json!({
        "chunk_set": current.chunk_set_id,
        "embedding_profile": current.embedding_profile,
        "sparse_profile": current.sparse_profile,
        "embedder_card": card,
    });
    (progress, authority)
}

#[derive(Clone, Copy)]
pub(super) struct RestoreFacts<'a> {
    pub(super) home: &'a Home,
    pub(super) job: Ulid,
    pub(super) generation: i64,
    pub(super) set_id: &'a str,
    pub(super) embedder: &'a Digest,
    pub(super) reranker: &'a Digest,
    pub(super) suite: &'a Digest,
    pub(super) outcome: &'a Value,
    pub(super) old_progress: &'a Value,
    pub(super) authority: &'a Value,
    pub(super) grants: &'a [String],
    pub(super) manifest: &'a Value,
}

pub(super) fn verify_restored_authority(facts: RestoreFacts<'_>) -> Value {
    let RestoreFacts {
        home,
        job,
        generation,
        set_id,
        embedder,
        reranker,
        suite,
        outcome,
        old_progress,
        authority,
        grants,
        manifest,
    } = facts;
    let database = home.database();
    let scopes = database.visible(LOCAL).unwrap();
    assert_eq!(grant_names(&scopes), grants);
    assert_eq!(
        database.chunk_set(&scopes, set_id).unwrap().unwrap().state,
        ChunkSetState::Complete
    );
    let original = database.generation(&scopes, generation).unwrap().unwrap();
    assert_eq!(original.state, GenerationState::Published);
    assert_eq!(original.chunk_set_id, set_id);
    assert_eq!(
        database
            .job(&scopes, job)
            .unwrap()
            .unwrap()
            .outcome
            .as_ref(),
        Some(outcome)
    );
    assert_eq!(
        &database.last_progress(&scopes, job).unwrap().unwrap().data,
        old_progress
    );
    assert!(!database.get(embedder).unwrap().is_empty());
    assert!(!database.get(reranker).unwrap().is_empty());
    assert!(!database.get(suite).unwrap().is_empty());
    for digest in [embedder, reranker, suite] {
        assert!(
            manifest["artifacts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["sha256"] == digest.as_str())
        );
        let bytes = Store::new(home.data().join("artifacts"))
            .get(digest)
            .unwrap();
        assert_eq!(Digest::of(&bytes), *digest);
    }
    assert_eq!(authority["chunk_set"], original.chunk_set_id);
    json!({"generation": original.id, "chunk_set": original.chunk_set_id})
}

pub(super) fn verify_backup(backup: &Path, required: &[Digest]) -> Value {
    let manifest = backup_restore::manifest(backup);
    assert_eq!(manifest["schema"], "maestro-backup/1");
    let db_path = backup.join("kernel.sqlite3");
    let db_bytes = fs::read(&db_path).unwrap();
    assert_eq!(
        manifest["database"]["sha256"],
        Digest::of(&db_bytes).as_str()
    );
    assert_eq!(manifest["database"]["size"], db_bytes.len());
    let artifacts = manifest["artifacts"].as_array().unwrap();
    assert!(!artifacts.is_empty());
    let mut digests = BTreeSet::new();
    for artifact in artifacts {
        let relative = Path::new(artifact["path"].as_str().unwrap());
        assert!(!relative.is_absolute());
        assert!(
            !relative
                .components()
                .any(|part| part == Component::ParentDir)
        );
        let bytes = fs::read(backup.join(relative)).unwrap();
        assert_eq!(artifact["sha256"], Digest::of(&bytes).as_str());
        assert_eq!(artifact["size"], bytes.len());
        digests.insert(artifact["sha256"].as_str().unwrap().to_owned());
    }
    for digest in required {
        assert!(
            digests.contains(digest.as_str()),
            "backup omits required artifact {digest:?}"
        );
    }
    manifest
}

pub(super) fn read_baseline(home: &Home, path: &Path, expected_digest: &Digest) -> Snapshot {
    let bytes = fs::read(path).unwrap();
    assert_eq!(Digest::of(&bytes), *expected_digest);
    let sidecar = fs::read_to_string(home.root().join("rankings-baseline.sha256")).unwrap();
    assert_eq!(sidecar, expected_digest.as_str());
    serde_json::from_slice(&bytes).unwrap()
}

pub(super) fn record_legacy_sentinels(home: &Home) {
    let data = home.data();
    fs::write(data.join("ledger.sqlite3"), b"legacy ledger").unwrap();
    fs::create_dir_all(data.join("material")).unwrap();
    fs::write(data.join("material/owner.txt"), b"legacy material").unwrap();
    fs::write(data.join("maestro.sock"), b"legacy socket marker").unwrap();
    fs::write(data.join("supervisor.lock"), b"legacy lock marker").unwrap();
}

pub(super) fn assert_legacy_sentinels(home: &Home) {
    let data = home.data();
    assert_eq!(
        fs::read(data.join("ledger.sqlite3")).unwrap(),
        b"legacy ledger"
    );
    assert_eq!(
        fs::read(data.join("material/owner.txt")).unwrap(),
        b"legacy material"
    );
    assert_eq!(
        fs::read(data.join("maestro.sock")).unwrap(),
        b"legacy socket marker"
    );
    assert_eq!(
        fs::read(data.join("supervisor.lock")).unwrap(),
        b"legacy lock marker"
    );
}

#[derive(Clone, Copy)]
pub(super) struct ProjectionExpectation<'a> {
    pub(super) home: &'a Home,
    pub(super) qdrant: &'a QdrantScratch,
    pub(super) generation: i64,
    pub(super) old_generation: i64,
    pub(super) chunk_set: &'a str,
    pub(super) baseline: &'a Snapshot,
}

pub(super) fn assert_projection_is_ready(expectation: ProjectionExpectation<'_>) {
    let ProjectionExpectation {
        home,
        qdrant,
        generation,
        old_generation,
        chunk_set,
        baseline,
    } = expectation;
    let database = home.database();
    let scopes = database.visible(LOCAL).unwrap();
    let current = database.generation(&scopes, generation).unwrap().unwrap();
    let old = database
        .generation(&scopes, old_generation)
        .unwrap()
        .unwrap();
    assert_eq!(current.state, GenerationState::Published);
    assert_eq!(old.state, GenerationState::Retired);
    assert_eq!(current.chunk_set_id, chunk_set);
    assert_eq!(
        current.embedding_profile,
        baseline.frozen.profiles["embedding"]
    );
    assert_eq!(current.sparse_profile, lexical::PROFILE);
    assert_eq!(
        current.point_count,
        Some(u64::try_from(baseline.frozen.point_ids.len()).unwrap())
    );
    let marker = database
        .generation_search(&scopes, generation)
        .unwrap()
        .unwrap();
    assert!(marker.ready);
    assert_eq!(marker.identifier_profile, IDENTIFIER_PROFILE);
    let aliases = qdrant.aliases().unwrap();
    assert_eq!(
        aliases.get(ALIAS),
        Some(&format!("maestro-synthetic-g{generation}"))
    );
    let collections = qdrant.collections().unwrap();
    assert!(collections.contains(&format!("maestro-synthetic-g{generation}")));
    assert!(!collections.contains(&format!("maestro-synthetic-g{old_generation}")));
}

pub(super) fn assert_point_payloads(
    qdrant: &QdrantScratch,
    home: &Home,
    generation: i64,
    baseline: &Snapshot,
) {
    let database = home.database();
    let scopes = database.visible(LOCAL).unwrap();
    let set = database
        .chunk_set(&scopes, &baseline.frozen.chunk_set)
        .unwrap()
        .unwrap();
    let chunks: HashMap<_, _> = database
        .chunks(&scopes, &set.id)
        .unwrap()
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();
    let revisions: HashMap<_, _> = database
        .revisions(&scopes, COLLECTION)
        .unwrap()
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();
    let generation_record = database.generation(&scopes, generation).unwrap().unwrap();
    let name = format!(
        "maestro-{}-g{}",
        generation_record.collection_id, generation_record.id
    );
    let points = qdrant.points_with_payload(&name).unwrap();
    assert_eq!(points.len(), chunks.len());
    for (_, payload) in points {
        let chunk_id = payload["chunk_id"].as_str().unwrap();
        let chunk = chunks.get(chunk_id).unwrap();
        assert_eq!(payload["revision_id"], chunk.revision_id);
        assert_eq!(payload["scope_tags"], json!(SCOPE_TAGS));
        assert_eq!(payload["identifier_profile"], IDENTIFIER_PROFILE);
        assert!(revisions.contains_key(&chunk.revision_id));
    }
}
