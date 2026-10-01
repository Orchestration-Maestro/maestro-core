//! Exact canonical-section retrieval across the CLI and MCP surfaces.

use super::super::support::{Home, local, synthetic};
use super::cli_cases::{
    SET_ID, SOURCE_REF, canonical_section_id, canonical_section_id_for_revision, document_id,
    mcp_tool_error, published_glossary, revision,
};
use maestro_kernel::{
    artifact::Digest,
    chunk_set::{Chunk, NewChunkSet},
    document::{Disposition, Outcome},
    evidence::Span,
    generation::NewGeneration,
};
use serde_json::{Value, json};
use std::{fs, io::Write};

const OTHER_SOURCE_REF: &str = "https://handbook.example.org/4.2/other-glossary";

#[test]
fn section_get_returns_the_authoritative_section_and_advertises_its_section_path() {
    let home = Home::new();
    let generation = published_glossary(&home, SET_ID, (0, None));
    let section_id = canonical_section_id(&home.database());
    let generation_arg = generation.to_string();
    let result = home.run(&[
        "--json",
        "knowledge",
        "get",
        "--collection",
        "synthetic",
        "--generation",
        &generation_arg,
        "--section-id",
        &section_id,
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
                    "section_id": section_id,
                    "document_id": document_id(&home.database()),
                    "revision_id": revision(&home.database()).id,
                    "source_ref": SOURCE_REF,
                    "title": "Glossary",
                    "version": "4.2",
                    "section_path": ["Glossary"],
                    "span": [0, source.len()],
                    "digest": format!("sha256:{}", Digest::of(source.as_bytes()).as_str()),
                    "text": source,
                }
            },
            "truncated": false,
            "limit_bytes": 65536,
        })
    );

    let help = home.run(&["knowledge", "get", "--help"]);
    assert_eq!(help.code, Some(0), "{help:?}");
    assert!(help.stdout.contains("--section-id"), "{help:?}");
}

#[test]
fn unknown_and_inaccessible_sections_have_one_privacy_safe_refusal() {
    let home = Home::new();
    published_glossary(&home, SET_ID, (0, None));
    let section_id = canonical_section_id(&home.database());
    home.configure("[access]\nread = []\n");
    let inaccessible = home.run(&["knowledge", "get", "--section-id", &section_id]);
    let unknown = home.run(&["knowledge", "get", "--section-id", "hidden-section"]);

    assert_eq!(inaccessible.code, Some(2), "{inaccessible:?}");
    assert_eq!(unknown.code, Some(2), "{unknown:?}");
    assert_eq!(inaccessible.stderr, unknown.stderr);
    assert_eq!(
        inaccessible.stderr.trim(),
        "section is unknown or not readable in the selected generation"
    );
    assert!(!inaccessible.stderr.contains("Glossary"));
}

#[test]
fn source_grants_read_only_their_sections_over_cli_and_mcp() {
    let home = Home::new();
    let (generation, handbook_section, other_section, source) = two_source_glossaries(&home);
    let generation_arg = generation.to_string();
    let cli_get = |section_id: &str| {
        home.run(&[
            "--json",
            "knowledge",
            "get",
            "--collection",
            "synthetic",
            "--generation",
            &generation_arg,
            "--section-id",
            section_id,
        ])
    };

    home.configure("[access]\nread = ['workspace/default/collection/synthetic/source/handbook']\n");
    let handbook = cli_get(&handbook_section);
    assert_eq!(handbook.code, Some(0), "{handbook:?}");
    assert_eq!(handbook.json()["data"]["excerpt"]["text"], source);
    assert_eq!(
        handbook.json()["data"]["excerpt"]["section_path"],
        json!(["Glossary"])
    );
    let mcp_handbook = mcp_section_get(&home, &handbook_section, generation);
    assert_eq!(mcp_handbook["result"]["isError"], false);
    assert_eq!(
        mcp_handbook["result"]["structuredContent"]["excerpt"]["text"],
        source
    );

    let hidden_other = cli_get(&other_section);
    assert_eq!(hidden_other.code, Some(2), "{hidden_other:?}");
    assert_eq!(
        hidden_other.stderr.trim(),
        "section is unknown or not readable in the selected generation"
    );
    let hidden_other_mcp = mcp_section_get(&home, &other_section, generation);
    let hidden_other_error = mcp_tool_error(&hidden_other_mcp);
    assert_eq!(hidden_other_error["error"]["code"], "not_found");
    assert_eq!(
        hidden_other_error["error"]["message"],
        hidden_other.stderr.trim()
    );

    home.configure("[access]\nread = ['workspace/default/collection/synthetic/source/other']\n");
    let denied_handbook = cli_get(&handbook_section);
    assert_eq!(denied_handbook.code, Some(2), "{denied_handbook:?}");
    assert_eq!(
        denied_handbook.stderr.trim(),
        "section is unknown or not readable in the selected generation"
    );
    let denied_handbook_mcp = mcp_section_get(&home, &handbook_section, generation);
    let denied_handbook_error = mcp_tool_error(&denied_handbook_mcp);
    assert_eq!(denied_handbook_error["error"]["code"], "not_found");
    assert_eq!(
        denied_handbook_error["error"]["message"],
        denied_handbook.stderr.trim()
    );
}

fn two_source_glossaries(home: &Home) -> (i64, String, String, String) {
    let source = fs::read_to_string(synthetic().join("corpus/en/glossary.md")).unwrap();
    add_two_source_collection(home, &source);
    let (generation, handbook_section, other_section) =
        publish_two_source_generation(home, &source);
    (generation, handbook_section, other_section, source)
}

fn add_two_source_collection(home: &Home, source: &str) {
    let root = home.root().join("two-source-corpus");
    let corpus = root.join("corpus");
    fs::create_dir_all(&corpus).unwrap();
    for (source_id, path, source_ref) in [
        ("handbook", "handbook.md", SOURCE_REF),
        ("other", "other.md", OTHER_SOURCE_REF),
    ] {
        fs::write(corpus.join(path), source).unwrap();
        let entry = json!({
            "schema": "maestro-corpus/1",
            "path": path,
            "sha256": Digest::of(source.as_bytes()).as_str(),
            "bytes": source.len(),
            "source_ref": source_ref,
            "title": "Glossary",
            "source_kind": "reference",
            "set": "operations",
            "version": "4.2",
        });
        fs::write(
            corpus.join(format!("{source_id}.jsonl")),
            format!("{entry}\n"),
        )
        .unwrap();
    }

    let mut declaration: Value =
        serde_json::from_slice(&fs::read(synthetic().join("collection.json")).unwrap()).unwrap();
    declaration["sources"] = json!([
        {
            "id": "handbook",
            "kind": "import",
            "sync": "manual",
            "manifest": {"binding": "synthetic_root", "path": "corpus/handbook.jsonl"}
        },
        {
            "id": "other",
            "kind": "import",
            "sync": "manual",
            "manifest": {"binding": "synthetic_root", "path": "corpus/other.jsonl"}
        }
    ]);
    let declaration_path = home.root().join("synthetic-two-sources.json");
    fs::write(&declaration_path, serde_json::to_vec(&declaration).unwrap()).unwrap();
    fs::write(
        home.config().join("bindings.toml"),
        format!("synthetic_root = '{}'\n", root.display()),
    )
    .unwrap();
    let added = home.run(&[
        "knowledge",
        "collection",
        "add",
        declaration_path.to_str().unwrap(),
    ]);
    assert_eq!(added.code, Some(0), "{added:?}");
    let imported = home.run(&["knowledge", "import", "--collection", "synthetic"]);
    assert_eq!(imported.code, Some(0), "{imported:?}");
}

fn publish_two_source_generation(home: &Home, source: &str) -> (i64, String, String) {
    let database = home.database();
    let scopes = local(&database);
    let revisions = database.revisions(&scopes, "synthetic").unwrap();
    let revision_for = |source_ref: &str| {
        revisions
            .iter()
            .find(|revision| {
                database
                    .document(&scopes, &revision.document_id)
                    .unwrap()
                    .is_some_and(|document| document.source_ref == source_ref)
            })
            .expect("source revision")
    };
    let handbook_revision = revision_for(SOURCE_REF);
    let other_revision = revision_for(OTHER_SOURCE_REF);
    for revision in [handbook_revision, other_revision] {
        database
            .record_disposition(&Disposition {
                revision_id: revision.id.clone(),
                outcome: Outcome::Accepted,
                reasons: Vec::new(),
                rule_ids: Vec::new(),
                decided_by: "test".to_owned(),
            })
            .unwrap();
    }
    let handbook_section = canonical_section_id_for_revision(&database, handbook_revision);
    let other_section = canonical_section_id_for_revision(&database, other_revision);
    let chunk_set_id = "set-two-source-sections";
    database
        .begin_chunk_set(&NewChunkSet {
            id: chunk_set_id,
            collection_id: "synthetic",
            chunk_profile: "structural-500-700/1",
            counter_contract_id: "test",
        })
        .unwrap();
    let digest = database.put(source.as_bytes(), "text/markdown").unwrap();
    for (source_id, revision, section_id) in [
        ("handbook", handbook_revision, &handbook_section),
        ("other", other_revision, &other_section),
    ] {
        database
            .record_chunks(
                chunk_set_id,
                &revision.id,
                &[Chunk {
                    id: format!("chunk-{source_id}"),
                    revision_id: revision.id.clone(),
                    section_id: Some(section_id.clone()),
                    digest: digest.clone(),
                    token_count: 1,
                    span: Span {
                        start: 0,
                        end: source.len(),
                    },
                }],
            )
            .unwrap();
    }
    let manifest = database.put(b"{}", "application/json").unwrap();
    database
        .complete_chunk_set(chunk_set_id, &manifest)
        .unwrap();
    let generation = database
        .create_generation(&NewGeneration {
            collection_id: "synthetic".to_owned(),
            chunk_set_id: chunk_set_id.to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap();
    database.verify_generation(generation.id, 2).unwrap();
    database.publish_generation(generation.id).unwrap();
    (generation.id, handbook_section, other_section)
}

fn mcp_section_get(home: &Home, section_id: &str, generation: i64) -> Value {
    let (server, mut input) = home.start_with_stdin(&["--json", "mcp"]);
    let requests = [
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "1"}
            }
        }),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "knowledge_get",
                "arguments": {
                    "section_id": section_id,
                    "collection": "synthetic",
                    "generation": generation
                }
            }
        }),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    input
        .write_all(format!("{requests}\n").as_bytes())
        .expect("write MCP request");
    drop(input);
    let output = server.finish();
    assert_eq!(output.code, Some(0), "{output:?}");
    assert_eq!(output.stderr, "");
    output
        .stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("parse MCP response"))
        .find(|response| response["id"] == 3)
        .expect("section get response")
}
