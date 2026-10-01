//! The public CLI refuses an invalid ask model before contacting a backend.

use super::support::Home;
use serde_json::{Value, json};
use std::io::Write as _;

#[test]
fn ask_reports_invalid_model_as_a_json_input_refusal() {
    let home = Home::bare();
    let result = home.run(&[
        "--json",
        "knowledge",
        "ask",
        "--collection",
        "docs",
        "--question",
        "How can I configure the service?",
        "--model",
        "invalid/model/entry",
    ]);

    assert_eq!(result.code, Some(2), "{result:?}");
    assert_eq!(result.stderr, "");
    assert_eq!(
        result.json(),
        serde_json::json!({
            "schema": "maestro-cli/knowledge-ask-error/1",
            "error": {
                "code": "invalid_arguments",
                "message": "model must be one router entry",
            }
        })
    );
}

#[test]
fn knowledge_ask_mcp_uses_the_cli_operation_refusal() {
    let home = Home::bare();
    let cli = home.run(&[
        "--json",
        "knowledge",
        "ask",
        "--collection",
        "docs",
        "--question",
        "How can I configure the service?",
        "--model",
        "invalid/model/entry",
    ]);
    assert_eq!(cli.code, Some(2), "{cli:?}");

    let (child, mut input) = home.start_with_stdin(&["--json", "mcp"]);
    input
        .write_all(
            concat!(
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{",
                "\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},",
                "\"clientInfo\":{\"name\":\"test\",\"version\":\"1\"}}}\n",
                "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{",
                "\"name\":\"knowledge_ask\",\"arguments\":{",
                "\"collection\":\"docs\",\"question\":\"How can I configure the service?\",",
                "\"model\":\"invalid/model/entry\"}}}\n"
            )
            .as_bytes(),
        )
        .unwrap();
    drop(input);
    let output = child.finish();
    assert_eq!(output.code, Some(0), "{output:?}");
    assert_eq!(output.stderr, "");
    let responses = output
        .stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let response = responses
        .iter()
        .find(|response| response["id"] == json!(2))
        .expect("knowledge_ask response");
    assert_eq!(response["result"]["isError"], true);
    let mcp_error: Value = serde_json::from_str(
        response["result"]["content"][0]["text"]
            .as_str()
            .expect("MCP error JSON"),
    )
    .expect("MCP error document");
    assert_eq!(mcp_error["error"], cli.json()["error"]);
}
