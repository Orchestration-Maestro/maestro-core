//! Exact `knowledge get` CLI behavior.

use super::super::support::{Ended, Home, bind_synthetic_corpus, local, synthetic};
use maestro_kernel::{
    artifact::Digest,
    chunk_set::{Chunk, NewChunkSet},
    document::{Disposition, Outcome, Revision},
    evidence::Span,
    generation::NewGeneration,
    store::Database,
};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

pub(super) const CHUNK_ID: &str = "chunk-glossary";
pub(super) const SET_ID: &str = "set-glossary";
pub(super) const SOURCE_REF: &str = "https://handbook.example.org/4.2/glossary";

#[test]
fn an_unknown_or_inaccessible_chunk_has_one_privacy_safe_refusal() {
    let home = Home::new();
    published_glossary(&home, SET_ID, (0, None));
    home.configure("[access]\nread = []\n");
    let inaccessible = home.run(&["knowledge", "get", "--chunk-id", CHUNK_ID]);
    let unknown = home.run(&["knowledge", "get", "--chunk-id", "hidden-id"]);

    assert_eq!(inaccessible.code, Some(2), "{inaccessible:?}");
    assert_eq!(unknown.code, Some(2), "{unknown:?}");
    assert_eq!(inaccessible.stdout, "", "{inaccessible:?}");
    assert_eq!(unknown.stdout, "", "{unknown:?}");
    assert_eq!(
        inaccessible.stderr.trim(),
        "chunk is unknown or not readable in the selected generation"
    );
    assert_eq!(inaccessible.stderr, unknown.stderr);
}

#[test]
fn human_get_refuses_oversized_metadata_to_stderr() {
    let home = Home::new();
    let source = fs::read(synthetic().join("corpus/en/glossary.md")).unwrap();
    let (source_ref, generation, _) = oversized_source_ref_generation(&home, &source);
    let generation = generation.to_string();
    let result = home.run(&[
        "knowledge",
        "get",
        "--chunk-id",
        CHUNK_ID,
        "--collection",
        "synthetic",
        "--generation",
        &generation,
    ]);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert_eq!(result.stdout, "");
    assert_eq!(
        result.stderr.trim(),
        "response_too_large: the exact excerpt exceeds the response limit"
    );
    assert!(!result.stderr.contains(&source_ref));
}

#[test]
fn oversized_sections_return_a_bounded_typed_refusal() {
    let home = Home::new();
    let source = format!("# Glossary\n\n{}", "x".repeat(65_536)).into_bytes();
    let (source_ref, generation, section_id) = oversized_source_ref_generation(&home, &source);
    let generation = generation.to_string();
    let result = home.run(&[
        "--json",
        "knowledge",
        "get",
        "--collection",
        "synthetic",
        "--generation",
        &generation,
        "--section-id",
        &section_id,
    ]);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert_eq!(result.json()["error"]["code"], "response_too_large");
    assert_eq!(result.json()["truncated"], true);
    assert_eq!(result.json()["omitted"], json!(["excerpt"]));
    assert!(result.stdout.len() <= 65_536);
    assert!(!result.stdout.contains(&source_ref));
}

#[test]
fn get_returns_exact_source_bytes_from_the_admitted_generation() {
    let home = Home::new();
    let generation = published_glossary(&home, SET_ID, (0, None));
    let generation_id = generation.to_string();
    let result = home.run(&[
        "--json",
        "knowledge",
        "get",
        "--collection",
        "synthetic",
        "--generation",
        &generation_id,
        "--chunk-id",
        CHUNK_ID,
    ]);
    assert_eq!(
        (result.code, result.stderr.as_str()),
        (Some(0), ""),
        "{result:?}"
    );
    let source = fs::read_to_string(synthetic().join("corpus/en/glossary.md")).unwrap();
    assert_eq!(
        result.json(),
        json!({
            "schema": "maestro-cli/knowledge-get/1",
            "data": {
                "schema": "maestro-knowledge-get/1",
                "collection": "synthetic",
                "generation": generation,
                "excerpt": {
                    "chunk_id": CHUNK_ID,
                    "document_id": document_id(&home.database()),
                    "revision_id": revision(&home.database()).id,
                    "section_id": canonical_section_id(&home.database()),
                    "source_ref": SOURCE_REF,
                    "title": "Glossary",
                    "version": "4.2",
                    "span": [0, source.len()],
                    "digest": format!("sha256:{}", Digest::of(source.as_bytes()).as_str()),
                    "text": source,
                }
            },
            "truncated": false,
            "limit_bytes": 65536,
        })
    );
}

#[test]
fn get_can_read_a_chunk_with_a_source_grant_without_exposing_the_collection_title() {
    let home = Home::new();
    published_glossary(&home, SET_ID, (0, None));
    home.configure("[access]\nread = ['workspace/default/collection/synthetic/source/handbook']\n");
    let listing = home.run(&["--json", "knowledge", "collections"]);
    assert_eq!(listing.code, Some(0), "{listing:?}");
    assert_eq!(listing.json()["data"]["collections"], json!([]));
    assert!(!listing.stdout.contains("Synthetic operations handbook"));

    let got = home.run(&["--json", "knowledge", "get", "--chunk-id", CHUNK_ID]);
    assert_eq!(got.code, Some(0), "{got:?}");
    assert_eq!(got.json()["data"]["excerpt"]["source_ref"], SOURCE_REF);
}

#[test]
fn an_explicit_retired_generation_keeps_its_original_chunk_span() {
    let home = Home::new();
    let older = published_glossary(&home, "set-glossary-old", (0, None));
    published_glossary(&home, "set-glossary-new", (0, Some(5)));
    let old_id = older.to_string();
    let result = home.run(&[
        "--json",
        "knowledge",
        "get",
        "--collection",
        "synthetic",
        "--generation",
        &old_id,
        "--chunk-id",
        CHUNK_ID,
    ]);
    assert_eq!(result.code, Some(0), "{result:?}");
    let source = fs::read_to_string(synthetic().join("corpus/en/glossary.md")).unwrap();
    assert_eq!(result.json()["data"]["generation"], older);
    assert_eq!(result.json()["data"]["excerpt"]["text"], source);
    assert_eq!(result.json()["data"]["excerpt"]["span"][1], source.len());
}

#[test]
fn generation_requires_a_collection_and_must_be_positive() {
    let home = Home::bare();
    let missing_collection =
        home.run(&["knowledge", "get", "--chunk-id", "x", "--generation", "1"]);
    assert_eq!(missing_collection.code, Some(2), "{missing_collection:?}");
    assert_eq!(missing_collection.stdout, "");
    assert!(
        missing_collection.stderr.contains("--collection"),
        "{missing_collection:?}"
    );
    let nonpositive = home.run(&[
        "knowledge",
        "get",
        "--chunk-id",
        "x",
        "--collection",
        "synthetic",
        "--generation",
        "0",
    ]);
    assert_eq!(nonpositive.code, Some(2), "{nonpositive:?}");
    assert_eq!(nonpositive.stdout, "");
    assert!(
        nonpositive.stderr.contains("generation must be positive"),
        "{nonpositive:?}"
    );
}

fn oversized_source_ref_generation(home: &Home, source: &[u8]) -> (String, i64, String) {
    home.add_synthetic();
    let root = home.root().join("oversized-ref-corpus");
    fs::create_dir_all(root.join("corpus/en")).unwrap();
    fs::write(root.join("corpus/en/glossary.md"), source).unwrap();
    let source_ref = format!("https://handbook.example.org/{}", "x".repeat(65_536));
    let entry = json!({
        "schema": "maestro-corpus/1",
        "path": "en/glossary.md",
        "sha256": Digest::of(source).as_str(),
        "bytes": source.len(),
        "source_ref": source_ref.clone(),
        "title": "Glossary",
        "source_kind": "reference",
        "set": "operations",
        "version": "4.2",
    });
    fs::write(
        root.join("corpus/maestro-corpus.jsonl"),
        format!("{entry}\n"),
    )
    .unwrap();
    fs::write(
        home.config().join("bindings.toml"),
        format!("synthetic_root = '{}'\n", root.display()),
    )
    .unwrap();
    let imported = home.run(&["knowledge", "import", "--collection", "synthetic"]);
    assert_eq!(imported.code, Some(0), "{imported:?}");

    let database = home.database();
    let scopes = local(&database);
    let revision = database
        .revisions(&scopes, "synthetic")
        .unwrap()
        .into_iter()
        .find(|revision| {
            database
                .document(&scopes, &revision.document_id)
                .unwrap()
                .is_some_and(|document| document.source_ref == source_ref)
        })
        .expect("large source reference revision");
    database
        .record_disposition(&Disposition {
            revision_id: revision.id.clone(),
            outcome: Outcome::Accepted,
            reasons: Vec::new(),
            rule_ids: Vec::new(),
            decided_by: "test".to_owned(),
        })
        .unwrap();
    let section_id = canonical_section_id_for_revision(&database, &revision);
    let digest = database.put(source, "text/markdown").unwrap();
    let set_id = "set-oversized-source-ref";
    database
        .begin_chunk_set(&NewChunkSet {
            id: set_id,
            collection_id: "synthetic",
            chunk_profile: "structural-500-700/1",
            counter_contract_id: "test",
        })
        .unwrap();
    database
        .record_chunks(
            set_id,
            &revision.id,
            &[Chunk {
                id: CHUNK_ID.to_owned(),
                revision_id: revision.id.clone(),
                section_id: Some(section_id.clone()),
                digest,
                token_count: 1,
                span: Span {
                    start: 0,
                    end: source.len(),
                },
            }],
        )
        .unwrap();
    let manifest = database.put(b"{}", "application/json").unwrap();
    database.complete_chunk_set(set_id, &manifest).unwrap();
    let generation = database
        .create_generation(&NewGeneration {
            collection_id: "synthetic".to_owned(),
            chunk_set_id: set_id.to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap();
    database.verify_generation(generation.id, 1).unwrap();
    database.publish_generation(generation.id).unwrap();
    (source_ref, generation.id, section_id)
}

pub(super) fn published_glossary(home: &Home, set_id: &str, span: (usize, Option<usize>)) -> i64 {
    glossary_generation(home, "synthetic", set_id, span, true)
}

pub(super) fn building_glossary(home: &Home, set_id: &str) -> i64 {
    glossary_generation(home, "synthetic", set_id, (0, None), false)
}

pub(super) fn published_glossary_in(home: &Home, collection: &str, set_id: &str) -> i64 {
    glossary_generation(home, collection, set_id, (0, None), true)
}

fn glossary_generation(
    home: &Home,
    collection: &str,
    set_id: &str,
    span: (usize, Option<usize>),
    publish: bool,
) -> i64 {
    if collection == "synthetic" {
        home.add_synthetic();
    } else {
        add_glossary_collection(home, collection);
    }
    bind_synthetic_corpus(home, &["en/glossary.md"]);
    let imported = home.run(&["knowledge", "import", "--collection", collection]);
    assert_eq!(imported.code, Some(0), "{imported:?}");
    let database = home.database();
    let revision = revision_in(&database, collection);
    database
        .record_disposition(&Disposition {
            revision_id: revision.id.clone(),
            outcome: Outcome::Accepted,
            reasons: Vec::new(),
            rule_ids: Vec::new(),
            decided_by: "test".to_owned(),
        })
        .unwrap();
    let text = fs::read_to_string(synthetic().join("corpus/en/glossary.md")).unwrap();
    let end = span.1.unwrap_or(text.len());
    let content = text.get(span.0..end).unwrap();
    let prepared = database.put(content.as_bytes(), "text/markdown").unwrap();
    database
        .begin_chunk_set(&NewChunkSet {
            id: set_id,
            collection_id: collection,
            chunk_profile: "structural-500-700/1",
            counter_contract_id: "test",
        })
        .unwrap();
    database
        .record_chunks(
            set_id,
            &revision.id,
            &[Chunk {
                id: CHUNK_ID.to_owned(),
                revision_id: revision.id.clone(),
                section_id: Some(canonical_section_id(&database)),
                digest: prepared,
                token_count: 1,
                span: Span { start: span.0, end },
            }],
        )
        .unwrap();
    let manifest = json!({
        "schema": "maestro-chunk-set/1",
        "collection": collection,
        "chunk_set": set_id,
        "chunk_profile": "structural-500-700/1",
        "preparation_profile": "canonicalization/1",
        "counter": "test",
        "revisions": [revision.id],
        "duplicates": {},
        "near_duplicate_groups": [],
        "refusals": [],
        "left_out": [],
        "chunks": 1,
        "tokens": 1,
    });
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    let manifest = database.put(&manifest_bytes, "application/json").unwrap();
    database.complete_chunk_set(set_id, &manifest).unwrap();
    let generation = database
        .create_generation(&NewGeneration {
            collection_id: collection.to_owned(),
            chunk_set_id: set_id.to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap();
    if publish {
        database.verify_generation(generation.id, 1).unwrap();
        database.publish_generation(generation.id).unwrap();
    }
    generation.id
}

pub(super) fn two_published_glossaries(home: &Home) {
    published_glossary(home, "set-synthetic", (0, None));
    published_glossary_in(home, "other", "set-other");
}

fn add_glossary_collection(home: &Home, collection: &str) {
    let declaration = fs::read_to_string(synthetic().join("collection.json"))
        .unwrap()
        .replace(
            "\"id\": \"synthetic\"",
            &format!("\"id\": \"{collection}\""),
        )
        .replace(
            "Synthetic operations handbook in French and English, written for public tests",
            &format!("{collection} glossary collection"),
        );
    let path = home.root().join(format!("{collection}.json"));
    fs::write(&path, declaration).unwrap();
    let added = home.run(&["knowledge", "collection", "add", path.to_str().unwrap()]);
    assert_eq!(added.code, Some(0), "{added:?}");
}

pub(super) fn revision(database: &Database) -> Revision {
    revision_in(database, "synthetic")
}

fn revision_in(database: &Database, collection: &str) -> Revision {
    let scopes = local(database);
    database
        .revisions(&scopes, collection)
        .unwrap()
        .into_iter()
        .find(|revision| {
            database
                .document(&scopes, &revision.document_id)
                .unwrap()
                .is_some_and(|document| document.source_ref == SOURCE_REF)
        })
        .unwrap()
}

pub(super) fn document_id(database: &Database) -> String {
    revision(database).document_id
}

pub(super) fn canonical_section_id(database: &Database) -> String {
    canonical_section_id_for_revision(database, &revision(database))
}

pub(super) fn canonical_section_id_for_revision(
    database: &Database,
    revision: &Revision,
) -> String {
    let canonical: Value =
        serde_json::from_slice(&database.get(&revision.canonical_digest).unwrap()).unwrap();
    canonical["sections"]
        .as_array()
        .and_then(|sections| {
            sections
                .iter()
                .find(|section| section["heading_path"] == json!(["Glossary"]))
        })
        .and_then(|section| section["section_id"].as_str())
        .expect("canonical Glossary section")
        .to_owned()
}

pub(super) fn mcp_tool_error(response: &Value) -> Value {
    assert_eq!(response["result"]["isError"], true);
    assert!(response["result"]["structuredContent"].is_null());
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("JSON tool-error text");
    serde_json::from_str(text).expect("structured tool-error document")
}

pub(super) fn stored_artifact(home: &Home, digest: &Digest) -> PathBuf {
    let mut characters = digest.as_str().chars();
    let first: String = characters.by_ref().take(2).collect();
    let second: String = characters.by_ref().take(2).collect();
    home.data()
        .join("artifacts")
        .join("sha256")
        .join(first)
        .join(second)
        .join(digest.as_str())
}

pub(super) fn assert_refusal(result: &Ended, code: i32, message: &str) {
    assert_eq!(result.code, Some(code), "{result:?}");
    assert_eq!(result.stdout, "", "{result:?}");
    assert_eq!(result.stderr.trim(), message, "{result:?}");
}
