//! Client-like MCP stdio round trips over the synthetic collection.

use super::{
    knowledge_get::cli_cases::{SET_ID, published_glossary},
    support::Home,
};
use serde_json::{Value, json};
use std::io::Write as _;

#[test]
fn documented_stdio_session_lists_tools_searches_and_refuses_an_ask_with_an_invalid_model() {
    let home = Home::new();
    published_glossary(&home, SET_ID, (0, None));
    let (child, mut input) = home.start_with_stdin(&["mcp"]);
    for request in [
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25", "capabilities": {},
                "clientInfo": {"name": "smoke-test", "version": "1"}
            }
        }),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": "knowledge_search", "arguments": {
                "collection": "synthetic", "query": "What does the glossary say?"
            }}
        }),
        json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": {"name": "knowledge_ask", "arguments": {
                "collection": "synthetic", "question": "What does the glossary say?",
                "model": "invalid/model/entry"
            }}
        }),
    ] {
        writeln!(input, "{request}").expect("write MCP request");
    }
    drop(input);

    let output = child.finish();
    assert_eq!(output.code, Some(0), "{output:?}");
    assert_eq!(output.stderr, "");
    let responses = output
        .stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("MCP response"))
        .collect::<Vec<_>>();
    assert_eq!(responses[0]["result"]["protocolVersion"], "2025-11-25");
    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|tool| tool["name"] == "knowledge_search"));
    assert!(tools.iter().any(|tool| tool["name"] == "knowledge_ask"));
    let search = responses
        .iter()
        .find(|response| response["id"] == 3)
        .unwrap();
    assert_eq!(search["result"]["isError"], false);
    assert_eq!(
        search["result"]["structuredContent"]["schema"],
        "maestro-evidence/1"
    );
    assert_eq!(
        search["result"]["structuredContent"]["collection"],
        "synthetic"
    );
    let ask = responses
        .iter()
        .find(|response| response["id"] == 4)
        .unwrap();
    assert_eq!(ask["result"]["isError"], true);
    assert!(
        ask["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("invalid_arguments")
    );
}
