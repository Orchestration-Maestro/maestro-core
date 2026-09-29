//! CLI and MCP search accept an evidence budget up to the 24,000-byte
//! ceiling and refuse one byte past it, naming the ceiling.

use super::super::support::{Ended, Home};
use super::cli_cases::{SET_ID, published_glossary};
use super::knowledge_search::{cli_data, mcp_search_result_of};
use serde_json::{Value, json};

const QUERY: &str = "What does the glossary say?";

/// A CLI search of the published glossary under `max_tokens`.
fn cli_search(home: &Home, max_tokens: &str) -> Ended {
    home.run(&[
        "--json",
        "knowledge",
        "search",
        "--collection",
        "synthetic",
        "--query",
        QUERY,
        "--max-tokens",
        max_tokens,
    ])
}

/// The result of an MCP search of the published glossary under `max_tokens`.
fn mcp_search(home: &Home, max_tokens: u32) -> Value {
    mcp_search_result_of(
        home.command(&["mcp"]),
        &json!({"collection": "synthetic", "query": QUERY, "max_tokens": max_tokens}),
    )
}

#[test]
fn cli_and_mcp_search_accept_the_24000_byte_evidence_ceiling() {
    let home = Home::new();
    published_glossary(&home, SET_ID, (0, None));

    let cli = cli_search(&home, "24000");
    let mcp = mcp_search(&home, 24_000);

    assert_eq!(cli.code, Some(0), "{cli:?}");
    let bundle = cli_data(&cli.stdout);
    assert_eq!(bundle["request_budget"]["max_tokens"], 24_000);
    assert_eq!(bundle["budget"]["limit"], 24_000);
    assert_eq!(mcp["isError"], false);
    assert_eq!(mcp["structuredContent"], bundle);
}

#[test]
fn cli_and_mcp_search_refuse_one_byte_past_the_ceiling_and_name_it() {
    let home = Home::bare();
    let expected = json!({
        "code": "invalid_max_tokens",
        "message": "max_tokens must be between 1 and 24000"
    });

    let cli = cli_search(&home, "24001");
    let mcp = mcp_search(&home, 24_001);

    assert_eq!(cli.code, Some(2), "{cli:?}");
    assert_eq!(cli.json()["error"], expected);
    assert_eq!(mcp["isError"], true);
    let mcp_error: Value = serde_json::from_str(
        mcp["content"][0]["text"]
            .as_str()
            .expect("typed search error"),
    )
    .expect("MCP error document");
    assert_eq!(mcp_error["error"], expected);
}
