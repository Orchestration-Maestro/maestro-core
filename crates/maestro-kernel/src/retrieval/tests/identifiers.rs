//! Literal identifier boundaries and shared whitespace normalization.

use crate::retrieval::{contains_identifier, normalize_whitespace};

#[test]
fn identifiers_match_only_exact_case_preserving_atoms() {
    for (text, identifier, normalized, expected) in [
        ("ERR-042.", "ERR-042", "ERR-042.", true),
        ("ERR-042", "ERR-042", "ERR-042", true),
        ("ERR-042.,", "ERR-042", "ERR-042.,", true),
        ("ERR-042.x", "ERR-042", "ERR-042.x", false),
        ("XERR-042x", "ERR-042", "XERR-042x", false),
        ("1.2.3", "1.2", "1.2.3", false),
        (
            "tool run --force --quiet",
            "tool run --force",
            "tool run --force --quiet",
            true,
        ),
        (
            "/prefix/config.xml",
            "config.xml",
            "/prefix/config.xml",
            false,
        ),
        ("APP=config.xml", "config.xml", "APP=config.xml", false),
        ("café", "cafe", "café", false),
        ("ERR-042\nnext", "ERR-042", "ERR-042 next", true),
        ("XERR-042 ERR-042", "ERR-042", "XERR-042 ERR-042", true),
        ("xa a a", "a a", "xa a a", true),
        ("", "", "", false),
    ] {
        assert_eq!(normalize_whitespace(text), normalized, "text: {text:?}");
        assert_eq!(
            contains_identifier(text, identifier),
            expected,
            "identifier {identifier:?} in {text:?}"
        );
    }
}

#[test]
fn identifier_edges_accept_only_the_specified_delimiters() {
    for delimiter in [
        "\t", "(", ")", "[", "]", "<", ">", ",", ";", ":", "!", "?", "\"", "'", "`", "|",
    ] {
        let text = format!("{delimiter}ERR-042{delimiter}");
        assert!(
            contains_identifier(&text, "ERR-042"),
            "delimiter: {delimiter:?}"
        );
    }
    assert!(contains_identifier("\u{2003}ERR-042\u{3000}", "ERR-042"));
    assert!(!contains_identifier("ERR-042.9", "ERR-042"));
}
