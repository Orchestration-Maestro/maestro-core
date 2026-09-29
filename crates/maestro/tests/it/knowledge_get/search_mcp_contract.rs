//! MCP search advertises only the strict read-only search arguments.

use super::super::support::Home;
use serde_json::{Value, json};
use std::io::Write as _;

#[test]
fn mcp_advertises_search_with_strict_object_arguments() {
    let home = Home::bare();
    let (server, mut input) = home.start_with_stdin(&["mcp"]);
    input
        .write_all(
            concat!(
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{",
                "\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},",
                "\"clientInfo\":{\"name\":\"test\",\"version\":\"1\"}}}\n",
                "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n"
            )
            .as_bytes(),
        )
        .expect("write MCP initialize and list");
    drop(input);
    let output = server.finish();
    assert_eq!(output.code, Some(0), "{output:?}");
    assert_eq!(output.stderr, "");
    let responses = output
        .stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("MCP JSON"))
        .collect::<Vec<_>>();
    let tools = responses[1]["result"]["tools"]
        .as_array()
        .expect("tools list");
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool["name"].as_str().expect("tool name"))
            .collect::<Vec<_>>(),
        [
            "knowledge_collections",
            "knowledge_get",
            "knowledge_search",
            "knowledge_ask",
        ]
    );
    let search = &tools[2];
    assert_eq!(search["inputSchema"]["type"], "object");
    assert_eq!(search["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        search["inputSchema"]["required"],
        json!(["collection", "query"])
    );
    assert_eq!(search["inputSchema"]["properties"]["k"]["minimum"], 1);
    assert_eq!(search["inputSchema"]["properties"]["k"]["maximum"], 50);
    assert_eq!(search["inputSchema"]["properties"]["k"]["default"], 10);
    assert_eq!(
        search["inputSchema"]["properties"]["evidence_bytes"]["maximum"],
        24_000
    );
    assert_eq!(
        search["inputSchema"]["properties"]["evidence_bytes"]["default"],
        6000
    );
    assert_eq!(
        search["inputSchema"]["properties"]["deadline_ms"]["maximum"],
        30_000
    );
    assert_eq!(
        search["inputSchema"]["properties"]["deadline_ms"]["default"],
        30_000
    );
}
