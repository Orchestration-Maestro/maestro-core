//! MCP parity, permission refresh, and exact-retrieval refusal cases.

use super::super::{
    mcp_stdio::assert_output_schema,
    support::{Home, synthetic},
};
use super::cli_cases::{
    CHUNK_ID, SET_ID, assert_refusal, building_glossary, mcp_tool_error, published_glossary,
    published_glossary_in, stored_artifact, two_published_glossaries,
};
use maestro_kernel::artifact::Digest;
use serde_json::{Value, json};
use std::{fs, io::Write as _};

#[test]
fn mcp_get_returns_the_same_data_as_cli_for_the_admitted_generation() {
    let home = Home::new();
    let generation = published_glossary(&home, SET_ID, (0, None));
    let generation_arg = generation.to_string();
    let cli = home.run(&[
        "--json",
        "knowledge",
        "get",
        "--collection",
        "synthetic",
        "--generation",
        &generation_arg,
        "--chunk-id",
        CHUNK_ID,
    ]);
    assert_eq!(cli.code, Some(0), "{cli:?}");

    let (server, mut input) = home.start_with_stdin(&["--json", "mcp"]);
    let requests = format!(
        concat!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{",
            "\"protocolVersion\":\"2025-11-25\",\"capabilities\":{{}},",
            "\"clientInfo\":{{\"name\":\"test\",\"version\":\"1\"}}}}}}\n",
            "{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}}\n",
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{{}}}}\n",
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{{",
            "\"name\":\"knowledge_get\",\"arguments\":{{\"chunk_id\":\"{}\",",
            "\"collection\":\"synthetic\",\"generation\":{}}}}}}}\n"
        ),
        CHUNK_ID, generation
    );
    input
        .write_all(requests.as_bytes())
        .expect("write MCP request");
    drop(input);
    let output = server.finish();
    assert_eq!(output.code, Some(0), "{output:?}");
    assert_eq!(output.stderr, "");
    let responses = output
        .stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("parse MCP response"))
        .collect::<Vec<_>>();
    let response = responses
        .iter()
        .find(|response| response["id"] == 3)
        .expect("get response");
    assert_eq!(response["result"]["isError"], false);
    assert_eq!(response["result"]["structuredContent"], cli.json()["data"]);
    let tools = responses
        .iter()
        .find(|response| response["id"] == 2)
        .expect("tool-list response");
    assert_output_schema(
        &response["result"]["structuredContent"],
        &tools["result"]["tools"][1]["outputSchema"],
    );
}

#[test]
fn access_revoked_between_mcp_calls_does_not_return_the_old_chunk() {
    let home = Home::new();
    let generation = published_glossary(&home, SET_ID, (0, None));
    let (mut server, mut input) = home.start_with_stdin(&["mcp"]);
    input
        .write_all(
            format!(
                concat!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{",
                    "\"protocolVersion\":\"2025-11-25\",\"capabilities\":{{}},",
                    "\"clientInfo\":{{\"name\":\"test\",\"version\":\"1\"}}}}}}\n",
                    "{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}}\n",
                    "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{{",
                    "\"name\":\"knowledge_get\",\"arguments\":{{\"chunk_id\":\"{}\",",
                    "\"collection\":\"synthetic\",\"generation\":{}}}}}}}\n"
                ),
                CHUNK_ID, generation
            )
            .as_bytes(),
        )
        .expect("write initial MCP request");
    let initialized: Value = serde_json::from_str(&server.line()).expect("parse initialize result");
    let first: Value = serde_json::from_str(&server.line()).expect("parse first get result");
    assert_eq!(initialized["id"], 1);
    assert_eq!(first["result"]["isError"], false);
    assert_eq!(
        first["result"]["structuredContent"]["generation"],
        generation
    );

    home.configure("[access]\nread = []\n");
    input
        .write_all(
            format!(
                concat!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{{",
                    "\"name\":\"knowledge_get\",\"arguments\":{{\"chunk_id\":\"{}\"}}}}}}\n"
                ),
                CHUNK_ID
            )
            .as_bytes(),
        )
        .expect("write call after revocation");
    let revoked: Value = serde_json::from_str(&server.line()).expect("parse revoked result");
    assert_eq!(
        mcp_tool_error(&revoked),
        json!({
            "schema": "maestro-mcp-error/1",
            "error": {
                "code": "not_found",
                "message": concat!(
                    "chunk is unknown or not readable in ",
                    "the selected generation"
                )
            },
            "truncated": false,
            "limit_bytes": 65536
        })
    );
    drop(input);
    let output = server.finish();
    assert_eq!(output.code, Some(0), "{output:?}");
    assert_eq!(output.stderr, "");
}

#[test]
fn chunk_id_utf8_byte_bounds_and_collection_names_have_exact_refusals() {
    let home = Home::bare();
    let blank = home.run(&["knowledge", "get", "--chunk-id", " \t "]);
    assert_refusal(
        &blank,
        2,
        "chunk_id must be nonblank and at most 256 UTF-8 bytes",
    );

    let boundary = "é".repeat(128);
    assert_eq!(boundary.len(), 256);
    let accepted = home.run(&["knowledge", "get", "--chunk-id", &boundary]);
    assert_refusal(
        &accepted,
        2,
        "chunk is unknown or not readable in the selected generation",
    );

    let oversized = format!("a{}", "é".repeat(128));
    assert_eq!(oversized.len(), 257);
    let rejected = home.run(&["knowledge", "get", "--chunk-id", &oversized]);
    assert_refusal(
        &rejected,
        2,
        "chunk_id must be nonblank and at most 256 UTF-8 bytes",
    );

    let invalid_collection = home.run(&[
        "knowledge",
        "get",
        "--chunk-id",
        "chunk",
        "--collection",
        "UPPER",
    ]);
    assert_refusal(
        &invalid_collection,
        2,
        "collection is not a valid kernel name",
    );
}

#[test]
fn mismatched_and_building_generations_share_the_unknown_chunk_refusal() {
    let home = Home::new();
    let synthetic = published_glossary(&home, "set-synthetic", (0, None));
    let other = published_glossary_in(&home, "other", "set-other");
    let building = building_glossary(&home, "set-building");

    for (collection, generation) in [
        ("synthetic", other),
        ("other", synthetic),
        ("synthetic", building),
    ] {
        let generation_arg = generation.to_string();
        let result = home.run(&[
            "knowledge",
            "get",
            "--chunk-id",
            CHUNK_ID,
            "--collection",
            collection,
            "--generation",
            &generation_arg,
        ]);
        assert_refusal(
            &result,
            2,
            "chunk is unknown or not readable in the selected generation",
        );
    }
}

#[test]
fn a_mutated_source_fails_closed_with_the_integrity_refusal() {
    let home = Home::new();
    published_glossary(&home, SET_ID, (0, None));
    let source = fs::read_to_string(synthetic().join("corpus/en/glossary.md"))
        .expect("read synthetic glossary");
    let digest = Digest::of(source.as_bytes());
    let corrupted = source.replace("Glossary", "Changed glossary");
    fs::write(stored_artifact(&home, &digest), corrupted).expect("corrupt stored source");

    let result = home.run(&["knowledge", "get", "--chunk-id", CHUNK_ID]);
    assert_refusal(&result, 1, "the source integrity check failed");
}

#[test]
fn ambiguous_chunk_ids_are_refused_with_an_exact_privacy_safe_message() {
    let home = Home::new();
    two_published_glossaries(&home);
    let ambiguous = home.run(&["knowledge", "get", "--chunk-id", CHUNK_ID]);
    assert_refusal(
        &ambiguous,
        2,
        concat!(
            "chunk matches more than one visible published generation; ",
            "specify collection and generation"
        ),
    );
}

#[test]
fn mixed_source_and_collection_grants_do_not_widen_metadata_visibility() {
    let home = Home::new();
    two_published_glossaries(&home);
    home.configure(
        "[access]\nread = ['workspace/default/collection/synthetic/source/handbook', \
         'workspace/default/collection/other']\n",
    );

    let listing = home.run(&["--json", "knowledge", "collections"]);
    assert_eq!(listing.code, Some(0), "{listing:?}");
    assert_eq!(listing.json()["data"]["collections"][0]["id"], "other");
    assert_eq!(
        listing.json()["data"]["collections"]
            .as_array()
            .expect("collection array")
            .len(),
        1
    );
    let got = home.run(&[
        "--json",
        "knowledge",
        "get",
        "--chunk-id",
        CHUNK_ID,
        "--collection",
        "synthetic",
    ]);
    assert_eq!(got.code, Some(0), "{got:?}");
    assert_eq!(got.json()["data"]["collection"], "synthetic");
}
