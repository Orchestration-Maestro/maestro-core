//! The stdio MCP server's process boundary and advertised tools.

use super::support::Home;
use serde_json::{Value, json};
use std::io::Write as _;

#[test]
fn mcp_initializes_and_lists_only_implemented_tools_without_cli_wrapping() {
    let home = Home::bare();
    let (child, mut input) = home.start_with_stdin(&["--json", "mcp"]);
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
        .unwrap();
    drop(input);
    let output = child.finish();
    let stdout = output.stdout;
    assert_eq!(output.code, Some(0), "{stdout} {}", output.stderr);
    assert_eq!(output.stderr, "", "MCP protocol output is stdout-only");
    let responses = stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 2, "{stdout}");
    assert_eq!(responses[0]["result"]["protocolVersion"], "2025-11-25");
    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["knowledge_collections", "knowledge_get"]
    );
    assert_collections_tool(&tools[0]);
    assert_get_tool(&tools[1]);
}

fn assert_collections_tool(collections: &Value) {
    assert_eq!(collections["inputSchema"]["type"], "object");
    assert_eq!(collections["inputSchema"]["additionalProperties"], false);
    assert_eq!(collections["inputSchema"]["properties"], json!({}));
    assert_eq!(collections["annotations"]["destructiveHint"], false);
    assert_eq!(collections["annotations"]["openWorldHint"], false);
    assert_eq!(collections["annotations"]["readOnlyHint"], true);
    assert!(collections["outputSchema"]["properties"]["truncated"].is_null());
    assert_eq!(
        collections["outputSchema"]["properties"]["collections"]["type"],
        "array"
    );
}

fn assert_get_tool(get: &Value) {
    assert_eq!(get["inputSchema"]["type"], "object");
    assert_eq!(get["inputSchema"]["additionalProperties"], false);
    assert_eq!(get["outputSchema"]["type"], "object");
    assert_eq!(
        get["inputSchema"]["properties"]["generation"]["minimum"],
        json!(1)
    );
    assert!(
        get["inputSchema"]["properties"]["chunk_id"]["description"]
            .as_str()
            .is_some_and(|description| description.contains("UTF-8 bytes"))
    );
    assert_eq!(get["annotations"]["destructiveHint"], false);
    assert_eq!(get["annotations"]["openWorldHint"], false);
}

#[test]
fn initialize_negotiates_rmcp_versions_and_accepts_a_follow_up_tools_list() {
    let home = Home::bare();
    for (requested, negotiated) in [
        ("2024-11-05", "2024-11-05"),
        ("2025-06-18", "2025-06-18"),
        ("2025-11-25", "2025-11-25"),
        ("2026-07-28", "2025-11-25"),
    ] {
        let (child, mut input) = home.start_with_stdin(&["mcp"]);
        let requests = format!(
            concat!(
                "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{",
                "\"protocolVersion\":\"{}\",\"capabilities\":{{}},",
                "\"clientInfo\":{{\"name\":\"test\",\"version\":\"1\"}}}}}}\n",
                "{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}}\n",
                "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{{}}}}\n"
            ),
            requested
        );
        input.write_all(requests.as_bytes()).unwrap();
        drop(input);
        let output = child.finish();
        assert_eq!(output.code, Some(0), "{requested}: {output:?}");
        assert_eq!(output.stderr, "", "{requested}");
        let responses = output
            .stdout
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        let initialize = responses
            .iter()
            .find(|response| response["id"] == 1)
            .unwrap();
        assert_eq!(
            initialize["result"]["protocolVersion"], negotiated,
            "requested {requested}"
        );
        let tools = responses
            .iter()
            .find(|response| response["id"] == 2)
            .unwrap();
        assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 2);
    }
}

#[test]
fn mcp_returns_bounded_tool_errors_and_protocol_errors_for_invalid_requests() {
    let home = Home::bare();
    let (child, mut input) = home.start_with_stdin(&["mcp"]);
    input
        .write_all(
            concat!(
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{",
                "\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},",
                "\"clientInfo\":{\"name\":\"test\",\"version\":\"1\"}}}\n",
                "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":99,\"method\":\n",
                "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{",
                "\"name\":\"knowledge_collections\",\"arguments\":{}}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{",
                "\"name\":\"knowledge_get\",\"arguments\":{\"section_id\":\"s1\"}}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{",
                "\"name\":\"knowledge_get\",\"arguments\":{\"chunk_id\":\"x\",",
                "\"principal\":\"other\"}}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\"params\":{",
                "\"name\":\"not_a_tool\",\"arguments\":{}}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"tools/call\",\"params\":{",
                "\"name\":\"knowledge_get\",\"arguments\":{\"chunk_id\":\"private\"}}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",\"params\":{",
                "\"name\":\"knowledge_collections\",\"arguments\":{\"principal\":\"other\"}}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"tools/call\",\"params\":{",
                "\"name\":\"knowledge_get\",\"arguments\":{\"chunk_id\":\"x\",",
                "\"generation\":7}}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"tools/list\",\"params\":{}}\n"
            )
            .as_bytes(),
        )
        .unwrap();
    drop(input);
    let output = child.finish();
    let stdout = output.stdout;
    assert_eq!(output.code, Some(0), "{stdout} {}", output.stderr);
    assert_eq!(output.stderr, "");
    let responses = stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let response = |id| {
        responses
            .iter()
            .find(|response| response["id"] == json!(id))
            .unwrap()
    };
    assert_eq!(
        response(2)["result"]["structuredContent"],
        json!({"schema": "maestro-knowledge-collections/1", "collections": []})
    );
    assert_eq!(response(2)["result"]["isError"], false);
    let tools = &response(8)["result"]["tools"];
    assert_output_schema(
        &response(2)["result"]["structuredContent"],
        &tools[0]["outputSchema"],
    );
    assert_bounded_tool_errors(&responses);
}

fn assert_bounded_tool_errors(responses: &[Value]) {
    let response = |id| {
        responses
            .iter()
            .find(|response| response["id"] == json!(id))
            .unwrap()
    };
    assert_eq!(
        tool_error(response(3)),
        json!({
            "schema": "maestro-mcp-error/1",
            "error": {
                "code": "section_id_unavailable",
                "message": concat!("section_id is not available ", "yet")
            },
            "truncated": false,
            "limit_bytes": 65536
        })
    );
    assert_eq!(
        tool_error(response(4)),
        json!({
            "schema": "maestro-mcp-error/1",
            "error": {
                "code": "invalid_arguments",
                "message": concat!("arguments do not match the ", "knowledge tool schema")
            },
            "truncated": false,
            "limit_bytes": 65536
        })
    );
    assert_eq!(
        tool_error(response(9))["error"],
        json!({
            "code": "generation_needs_collection",
            "message": "generation requires collection"
        })
    );
    assert_eq!(response(5)["error"]["code"], -32602);
    assert!(
        responses.iter().any(|response| {
            response["id"] == Value::Null && response["error"]["code"] == -32700
        })
    );
    assert_eq!(
        tool_error(response(6)),
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
    assert_eq!(
        tool_error(response(7))["error"]["code"],
        "invalid_arguments"
    );
}

pub(super) fn assert_output_schema(value: &Value, schema: &Value) {
    assert!(
        schema_matches(value, schema, schema),
        "value does not match {schema}"
    );
}

fn schema_matches(value: &Value, schema: &Value, root: &Value) -> bool {
    if let Some(reference) = schema["$ref"].as_str() {
        return reference
            .strip_prefix('#')
            .and_then(|pointer| root.pointer(pointer))
            .is_some_and(|resolved| schema_matches(value, resolved, root));
    }
    if let Some(alternatives) = schema["anyOf"]
        .as_array()
        .or_else(|| schema["oneOf"].as_array())
    {
        return alternatives
            .iter()
            .any(|alternative| schema_matches(value, alternative, root));
    }
    match schema["type"].as_str() {
        Some("object") => {
            let Some(object) = value.as_object() else {
                return false;
            };
            let Some(properties) = schema["properties"].as_object() else {
                return true;
            };
            schema["required"].as_array().is_none_or(|required| {
                required
                    .iter()
                    .all(|key| key.as_str().is_some_and(|key| object.contains_key(key)))
            }) && (schema["additionalProperties"] != false
                || object.keys().all(|key| properties.contains_key(key)))
                && object.iter().all(|(key, value)| {
                    properties
                        .get(key)
                        .is_none_or(|property| schema_matches(value, property, root))
                })
        }
        Some("array") => {
            let Some(items) = value.as_array() else {
                return false;
            };
            let item_count = u64::try_from(items.len()).unwrap_or(u64::MAX);
            if schema["minItems"]
                .as_u64()
                .is_some_and(|minimum| item_count < minimum)
                || schema["maxItems"]
                    .as_u64()
                    .is_some_and(|maximum| item_count > maximum)
            {
                return false;
            }
            let Some(item_schema) = schema.get("items") else {
                return true;
            };
            items
                .iter()
                .all(|item| schema_matches(item, item_schema, root))
        }
        Some("string") => value.is_string(),
        Some("integer") => value.as_i64().is_some_and(|integer| {
            schema["minimum"]
                .as_i64()
                .is_none_or(|minimum| integer >= minimum)
                && schema["maximum"]
                    .as_i64()
                    .is_none_or(|maximum| integer <= maximum)
        }),
        Some("number") => value.is_number(),
        Some("boolean") => value.is_boolean(),
        Some("null") => value.is_null(),
        _ => schema["type"].as_array().is_some_and(|types| {
            types
                .iter()
                .any(|kind| schema_matches(value, &json!({"type": kind}), root))
        }),
    }
}

fn tool_error(response: &Value) -> Value {
    assert_eq!(response["result"]["isError"], true);
    assert!(response["result"]["structuredContent"].is_null());
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("JSON tool-error text");
    serde_json::from_str(text).expect("structured tool-error document")
}

#[test]
fn mcp_rejects_long_string_request_ids_without_echoing_them() {
    let home = Home::bare();
    let (child, mut input) = home.start_with_stdin(&["mcp"]);
    let long_id = "r".repeat(257);
    let requests = format!(
        concat!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{",
            "\"protocolVersion\":\"2025-11-25\",\"capabilities\":{{}},",
            "\"clientInfo\":{{\"name\":\"test\",\"version\":\"1\"}}}}}}\n",
            "{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}}\n",
            "{{\"jsonrpc\":\"2.0\",\"id\":\"{}\",\"method\":\"tools/list\",\"params\":{{}}}}\n",
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{{}}}}\n"
        ),
        long_id
    );
    input.write_all(requests.as_bytes()).unwrap();
    drop(input);
    let output = child.finish();
    let stdout = output.stdout;
    assert_eq!(output.code, Some(0), "{stdout} {}", output.stderr);
    assert_eq!(output.stderr, "");
    let responses = stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert!(
        responses.iter().any(|response| {
            response["id"] == Value::Null && response["error"]["code"] == -32600
        })
    );
    assert!(responses.iter().any(|response| response["id"] == 2));
    assert!(!stdout.contains(&long_id));
}
