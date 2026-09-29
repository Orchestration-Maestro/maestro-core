//! CLI and MCP search through the shared scoped evidence pipeline.

use super::super::support::{Home, Running, local};
use super::cli_cases::{
    SET_ID, SOURCE_REF, canonical_section_id_for_revision, published_glossary, revision,
};
use maestro_kernel::{
    artifact::Digest,
    chunk_set::{Chunk, NewChunkSet},
    document::{Disposition, Outcome, Revision},
    evidence::Span,
    generation::NewGeneration,
    retrieval::{IDENTIFIER_PROFILE, SearchInput, SearchMember},
    scope::ScopeSet,
    store::Database,
};
use serde_json::{Value, json};
use std::{fs, io::Write as _, process::Command};

const QDRANT_URL: &str = "http://127.0.0.1:16634";
const ROUTER_URL: &str = "http://127.0.0.1:16633";
const QUERY: &str = "What does the glossary say?";
const IDENTIFIER_SOURCE: &str =
    "# Glossary\n\nRun `maestro search --force` to restore the archive.\n";
const IDENTIFIER_SET_ID: &str = "set-identifier-search";

#[test]
fn cli_search_returns_an_evidence_bundle_with_the_default_budget() {
    let home = Home::new();
    let generation = published_glossary(&home, SET_ID, (0, None));
    let mut command = home.command(&[
        "--json",
        "knowledge",
        "search",
        "--collection",
        "synthetic",
        "--query",
        QUERY,
    ]);
    command
        .env("MAESTRO_QDRANT_URL", QDRANT_URL)
        .env("MAESTRO_ROUTER_URL", ROUTER_URL);
    let result = Running::of(command).finish();

    assert_eq!(result.code, Some(0), "{result:?}");
    assert_eq!(result.stderr, "");
    assert!(
        result.stdout.len() <= 65_536,
        "{} bytes",
        result.stdout.len()
    );
    let output: Value = serde_json::from_str(result.stdout.trim()).expect("search JSON");
    assert_eq!(output["schema"], "maestro-cli/knowledge-search/1");
    assert_eq!(output["data"]["schema"], "maestro-evidence/1");
    assert_eq!(output["data"]["generation"], generation);
    assert_eq!(output["data"]["query"], QUERY);
    assert_eq!(
        output["data"]["request_budget"],
        json!({"k": 10, "evidence_bytes": 12_000, "deadline_ms": 30_000})
    );
    assert_eq!(output["data"]["budget"]["counter"], "evidence-utf8-bytes/1");
    assert_eq!(output["data"]["budget"]["estimated"], true);
    assert_eq!(output["truncated"], false);
}

#[test]
fn mcp_search_returns_the_same_bundle_as_the_cli() {
    let home = Home::new();
    let generation = published_glossary(&home, SET_ID, (0, None));
    let mut command = home.command(&[
        "--json",
        "knowledge",
        "search",
        "--collection",
        "synthetic",
        "--query",
        QUERY,
    ]);
    command
        .env("MAESTRO_QDRANT_URL", QDRANT_URL)
        .env("MAESTRO_ROUTER_URL", ROUTER_URL);
    let cli = Running::of(command).finish();
    assert_eq!(cli.code, Some(0), "{cli:?}");

    let (server, mut input) = home.start_with_stdin(&["mcp"]);
    let request = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "knowledge_search",
            "arguments": {"collection": "synthetic", "query": QUERY}
        }
    });
    let initialize = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1"}
        }
    });
    let list = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}});
    for message in [
        initialize,
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        list,
        request,
    ] {
        writeln!(input, "{message}").expect("write MCP search request");
    }
    drop(input);
    let output = server.finish();
    assert_eq!(output.code, Some(0), "{output:?}");
    assert_eq!(output.stderr, "");
    let responses = output
        .stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("MCP response"))
        .collect::<Vec<_>>();
    let search = responses
        .iter()
        .find(|response| response["id"] == 3)
        .expect("search response");
    assert_eq!(search["result"]["isError"], false);
    assert_eq!(search["result"]["structuredContent"], cli_data(&cli.stdout));
    let tool = responses[1]["result"]["tools"]
        .as_array()
        .expect("tools list")
        .iter()
        .find(|tool| tool["name"] == "knowledge_search")
        .expect("search tool");
    super::super::mcp_stdio::assert_output_schema(
        &search["result"]["structuredContent"],
        &tool["outputSchema"],
    );
    assert_eq!(
        search["result"]["structuredContent"]["generation"],
        generation
    );
}

#[test]
fn source_only_grants_and_hidden_collections_have_the_same_safe_search_refusal() {
    let home = Home::new();
    published_glossary(&home, SET_ID, (0, None));
    home.configure("[access]\nread = ['workspace/default/collection/synthetic/source/handbook']\n");

    let visible_in_collections = home.run(&["--json", "knowledge", "collections"]);
    assert_eq!(
        visible_in_collections.code,
        Some(0),
        "{visible_in_collections:?}"
    );
    assert_eq!(
        visible_in_collections.json()["data"]["collections"],
        json!([])
    );

    let cli_hidden = cli_search_error(&home, "synthetic");
    let cli_unknown = cli_search_error(&home, "missing");
    assert_eq!(cli_hidden["error"]["code"], "not_found");
    assert_eq!(cli_hidden["error"], cli_unknown["error"]);
    assert!(!cli_hidden.to_string().contains("synthetic"));

    let mcp_hidden = mcp_search_error(&home, &json!({"collection": "synthetic", "query": QUERY}));
    let mcp_unknown = mcp_search_error(&home, &json!({"collection": "missing", "query": QUERY}));
    assert_eq!(mcp_hidden["error"]["code"], "not_found");
    assert_eq!(mcp_hidden, mcp_unknown);
    assert!(!mcp_hidden.to_string().contains("synthetic"));
}

#[test]
fn malformed_mcp_search_arguments_are_typed_tool_errors() {
    let home = Home::bare();
    let response = mcp_search_error(&home, &json!({"collection": "synthetic", "query": 1}));
    assert_eq!(response["error"]["code"], "invalid_arguments");
}

#[test]
fn indexed_identifier_passage_is_identical_across_cli_and_mcp() {
    let home = Home::new();
    let generation_id = published_identifier_source(&home);
    let query = "What does --force do?";
    let cli = cli_search(&home, query);
    assert_eq!(cli.code, Some(0), "{cli:?}");
    let cli_bundle = cli_data(&cli.stdout);
    assert_eq!(cli_bundle["generation"], generation_id);
    assert_eq!(cli_bundle["passages"].as_array().unwrap().len(), 1);

    let mcp = mcp_search_result(&home, &json!({"collection": "synthetic", "query": query}));
    assert_eq!(mcp["isError"], false);
    assert_eq!(mcp["structuredContent"], cli_bundle);
    let text_bundle: Value = serde_json::from_str(mcp["content"][0]["text"].as_str().unwrap())
        .expect("reduced MCP text bundle");
    assert_eq!(text_bundle, cli_bundle);
}

fn cli_search(home: &Home, query: &str) -> super::super::support::Ended {
    home.run(&[
        "--json",
        "knowledge",
        "search",
        "--collection",
        "synthetic",
        "--query",
        query,
    ])
}

fn cli_search_error(home: &Home, collection: &str) -> Value {
    let result = home.run(&[
        "--json",
        "knowledge",
        "search",
        "--collection",
        collection,
        "--query",
        QUERY,
    ]);
    assert_eq!(result.code, Some(2), "{result:?}");
    result.json()
}

fn mcp_search_error(home: &Home, arguments: &Value) -> Value {
    let result = mcp_search_result(home, arguments);
    assert_eq!(result["isError"], true);
    serde_json::from_str(
        result["content"][0]["text"]
            .as_str()
            .expect("typed search error"),
    )
    .expect("MCP error document")
}

fn mcp_search_result(home: &Home, arguments: &Value) -> Value {
    mcp_search_result_of(home.command(&["mcp"]), arguments)
}

/// The result of one `knowledge_search` call with `arguments` to the MCP
/// server `command` starts.
pub(super) fn mcp_search_result_of(command: Command, arguments: &Value) -> Value {
    let (server, mut input) = Running::with_stdin(command);
    let initialize = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1"}
        }
    });
    let request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {"name": "knowledge_search", "arguments": arguments}
    });
    writeln!(input, "{initialize}").expect("write initialize");
    writeln!(
        input,
        "{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}}"
    )
    .expect("write initialized notification");
    writeln!(input, "{request}").expect("write search");
    drop(input);
    let output = server.finish();
    assert_eq!(output.code, Some(0), "{output:?}");
    let response = output
        .stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("MCP response"))
        .find(|response| response["id"] == 2)
        .expect("search response");
    response["result"].clone()
}

pub(super) fn published_identifier_source(home: &Home) -> i64 {
    home.add_synthetic();
    write_identifier_corpus(home);

    let database = home.database();
    let revision = revision(&database);
    database
        .record_disposition(&Disposition {
            revision_id: revision.id.clone(),
            outcome: Outcome::Accepted,
            reasons: Vec::new(),
            rule_ids: Vec::new(),
            decided_by: "test".to_owned(),
        })
        .expect("accept test source");
    let section_id = canonical_section_id_for_revision(&database, &revision);
    database
        .begin_chunk_set(&NewChunkSet {
            id: IDENTIFIER_SET_ID,
            collection_id: "synthetic",
            chunk_profile: "structural-500-700/1",
            counter_contract_id: "test",
        })
        .expect("begin chunk set");
    let scopes = local(&database);
    let search_inputs = record_identifier_chunks(&database, &revision, &section_id);
    publish_identifier_generation(&database, &scopes, &revision, &search_inputs)
}

fn write_identifier_corpus(home: &Home) {
    let root = home.root().join("identifier-search-corpus");
    let corpus = root.join("corpus/en");
    fs::create_dir_all(&corpus).expect("create corpus");
    fs::write(corpus.join("glossary.md"), IDENTIFIER_SOURCE).expect("write source");
    let entry = json!({
        "schema": "maestro-corpus/1",
        "path": "en/glossary.md",
        "sha256": Digest::of(IDENTIFIER_SOURCE.as_bytes()).as_str(),
        "bytes": IDENTIFIER_SOURCE.len(),
        "source_ref": SOURCE_REF,
        "title": "Glossary",
        "source_kind": "reference",
        "set": "operations",
        "version": "4.2"
    });
    fs::write(
        root.join("corpus/maestro-corpus.jsonl"),
        format!("{entry}\n"),
    )
    .expect("write manifest");
    fs::write(
        home.config().join("bindings.toml"),
        format!("synthetic_root = '{}'\n", root.display()),
    )
    .expect("bind corpus");
    let imported = home.run(&["knowledge", "import", "--collection", "synthetic"]);
    assert_eq!(imported.code, Some(0), "{imported:?}");
}

fn record_identifier_chunks(
    database: &Database,
    revision: &Revision,
    section_id: &str,
) -> Vec<SearchInput> {
    let mut chunks = Vec::new();
    let mut search_inputs = Vec::new();
    for index in 0..10 {
        let prepared = if index == 0 {
            IDENTIFIER_SOURCE.to_owned()
        } else {
            format!("Unrelated prepared input {index}.")
        };
        let digest = database
            .put(prepared.as_bytes(), "text/markdown")
            .expect("store prepared input");
        let chunk_id = format!("chunk-identifier-search-{index}");
        chunks.push(Chunk {
            id: chunk_id.clone(),
            revision_id: revision.id.clone(),
            section_id: Some(section_id.to_owned()),
            digest,
            token_count: 1,
            span: Span {
                start: 0,
                end: IDENTIFIER_SOURCE.len(),
            },
        });
        search_inputs.push(SearchInput {
            chunk_id,
            prepared_input: prepared,
            identifiers: if index == 0 {
                vec!["--force".to_owned()]
            } else {
                Vec::new()
            },
        });
    }
    database
        .record_chunks(IDENTIFIER_SET_ID, &revision.id, &chunks)
        .expect("record chunks");
    search_inputs
}

fn publish_identifier_generation(
    database: &Database,
    scopes: &ScopeSet,
    revision: &Revision,
    search_inputs: &[SearchInput],
) -> i64 {
    let chunk_manifest = json!({
        "schema": "maestro-chunk-set/1",
        "collection": "synthetic",
        "chunk_set": IDENTIFIER_SET_ID,
        "chunk_profile": "structural-500-700/1",
        "preparation_profile": "canonical-context-parts/v1",
        "counter": "test",
        "revisions": [revision.id],
        "duplicates": {},
        "near_duplicate_groups": [],
        "refusals": [],
        "left_out": [],
        "chunks": 10,
        "tokens": 10
    });
    let manifest_bytes = serde_json::to_vec(&chunk_manifest).expect("encode chunk manifest");
    let manifest = database
        .put(&manifest_bytes, "application/json")
        .expect("store chunk manifest");
    database
        .complete_chunk_set(IDENTIFIER_SET_ID, &manifest)
        .expect("complete chunk set");
    let generation = database
        .create_generation(&NewGeneration {
            collection_id: "synthetic".to_owned(),
            chunk_set_id: IDENTIFIER_SET_ID.to_owned(),
            embedding_profile: "embed:test".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .expect("create generation");
    database
        .begin_generation_search(scopes, generation.id, IDENTIFIER_PROFILE)
        .expect("begin identifier projection");
    database
        .record_search_members(
            scopes,
            IDENTIFIER_SET_ID,
            &[SearchMember {
                revision_id: revision.id.clone(),
                representative_revision_id: revision.id.clone(),
            }],
        )
        .expect("record search membership");
    database
        .record_search_inputs(scopes, IDENTIFIER_SET_ID, search_inputs)
        .expect("record identifier inputs");
    database
        .complete_generation_search(scopes, generation.id)
        .expect("complete identifier projection");
    database
        .verify_generation(generation.id, 1)
        .expect("verify generation");
    database
        .publish_generation(generation.id)
        .expect("publish generation");
    generation.id
}

pub(super) fn cli_data(stdout: &str) -> Value {
    serde_json::from_str::<Value>(stdout.trim()).expect("CLI search JSON")["data"].clone()
}
