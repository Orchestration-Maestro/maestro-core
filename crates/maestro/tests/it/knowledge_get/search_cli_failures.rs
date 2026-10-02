//! CLI search validation retains its documented process exit code.
//!
//! Deadline failures are covered by the CLI execution mapping test and the
//! knowledge library's paused-clock blocking evidence-counter test.

use super::super::support::Home;

const QUERY: &str = "What does the glossary say?";

#[test]
fn cli_search_validation_errors_preserve_exit_codes() {
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
}
