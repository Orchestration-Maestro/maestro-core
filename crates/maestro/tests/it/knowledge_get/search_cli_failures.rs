//! CLI search failures retain their documented process exit codes.

use super::super::support::Home;
use super::cli_cases::{SET_ID, published_glossary};

const QUERY: &str = "What does the glossary say?";

#[test]
fn cli_search_validation_and_deadline_errors_preserve_exit_codes() {
    let invalid_home = Home::bare();
    let invalid = invalid_home.run(&[
        "--json",
        "knowledge",
        "search",
        "--collection",
        "synthetic",
        "--query",
        QUERY,
        "--k",
        "0",
    ]);
    assert_eq!(invalid.code, Some(2), "{invalid:?}");
    assert_eq!(invalid.json()["error"]["code"], "invalid_k");

    let home = Home::new();
    published_glossary(&home, SET_ID, (0, None));
    let expired = home.run(&[
        "--json",
        "knowledge",
        "search",
        "--collection",
        "synthetic",
        "--query",
        QUERY,
        "--deadline-ms",
        "1",
    ]);
    assert_eq!(expired.code, Some(1), "{expired:?}");
    assert_eq!(expired.json()["error"]["code"], "deadline_exceeded");
}
