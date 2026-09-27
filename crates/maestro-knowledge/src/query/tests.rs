//! Regression tests for deterministic query understanding.

use super::{Family, Identifier, Language, QueryKind, understand};

fn assert_family_found(text: &str, family: Family, expected: &str) {
    let matching = understand(text)
        .identifiers
        .into_iter()
        .filter(|identifier| identifier.family == family)
        .collect::<Vec<_>>();
    assert_eq!(
        matching,
        vec![Identifier {
            family,
            text: expected.to_owned(),
        }],
        "query: {text:?}"
    );
}

fn assert_family_absent(text: &str, family: Family) {
    assert!(
        understand(text)
            .identifiers
            .iter()
            .all(|identifier| identifier.family != family),
        "query: {text:?}"
    );
}

#[test]
fn parameter_identifiers_match_flags_assignments_and_variables() {
    for (text, expected) in [
        ("--long-flag", "--long-flag"),
        ("-f", "-f"),
        ("OPTION=value", "OPTION=value"),
        ("$MODE", "$MODE"),
        ("${ROOT_DIR}", "${ROOT_DIR}"),
        ("_VALUE=1", "_VALUE=1"),
        ("$_MODE", "$_MODE"),
        ("Set OPTION=value.", "OPTION=value"),
        ("-q est visible en français", "-q"),
    ] {
        assert_family_found(text, Family::Parameter, expected);
    }
    for text in ["--", "VALUE=", "$", "a-b", "123=value", "caféNAME=value"] {
        assert_family_absent(text, Family::Parameter);
    }
}

#[test]
fn error_code_identifiers_match_only_the_documented_shape() {
    for (text, expected) in [
        ("ERR-042", "ERR-042"),
        ("ABC1234", "ABC1234"),
        ("X_12A", "X_12A"),
        ("Une erreur ERR-042 apparaît", "ERR-042"),
    ] {
        assert_family_found(text, Family::ErrorCode, expected);
    }
    for text in ["err-042", "ABC-1", "ABC1234567", "xERR-042"] {
        assert_family_absent(text, Family::ErrorCode);
    }
}

#[test]
fn path_identifiers_match_unix_windows_and_filenames() {
    for (text, expected) in [
        ("Open /etc/app.conf.", "/etc/app.conf"),
        ("Open ./a/b now", "./a/b"),
        ("Use ~/x today", "~/x"),
        (r"Open C:\a now", r"C:\a"),
        (r"Open C:\a\b now", r"C:\a\b"),
        ("Open C:\\a\\b.", "C:\\a\\b"),
        ("Ouvrez config.xml", "config.xml"),
    ] {
        assert_family_found(text, Family::Path, expected);
    }
    for text in [
        "plain-name",
        "a/b",
        "./",
        "/",
        "A?\\a",
        "A:/a",
        "C:/a",
        "C:\\",
    ] {
        assert_family_absent(text, Family::Path);
    }
}

#[test]
fn port_identifiers_accept_only_valid_port_numbers() {
    for (text, expected) in [
        ("port 8443", "8443"),
        ("service:443", "443"),
        ("Le service utilise port 65535", "65535"),
        ("host:8443-4", "8443"),
        ("port 1", "1"),
    ] {
        assert_family_found(text, Family::Port, expected);
    }
    for text in [
        "port 0",
        "service:65536",
        "port 8443x",
        "host:8443.2",
        "support 8443",
    ] {
        assert_family_absent(text, Family::Port);
    }
}

#[test]
fn version_identifiers_match_supported_numeric_versions() {
    for (text, expected) in [
        ("v2", "v2"),
        ("2.4", "2.4"),
        ("release 9.1.2", "9.1.2"),
        ("La version 3.4 est récente", "3.4"),
        ("v1.2.3.4", "v1.2.3.4"),
    ] {
        assert_family_found(text, Family::Version, expected);
    }
    for text in [
        "version 42",
        "V2",
        "v1.2.3.4.5",
        "2.4.5.6.7",
        "2.4x",
        "release 1. 2",
    ] {
        assert_family_absent(text, Family::Version);
    }
}

#[test]
fn command_identifiers_match_code_spans_and_commands_with_parameters() {
    for (text, expected) in [
        ("Run `runner --quiet` now", "runner --quiet"),
        ("tool run --force", "tool run --force"),
        ("outil lancer --force", "outil lancer --force"),
        ("tool-v2 --force", "tool-v2 --force"),
        ("tool_v2 --force", "tool_v2 --force"),
        ("tool run-2 --force", "tool run-2 --force"),
        ("tool run_2 --force", "tool run_2 --force"),
        ("tool run --force --quiet", "tool run --force --quiet"),
        ("Use `config.xml`", "config.xml"),
    ] {
        assert_family_found(text, Family::Command, expected);
    }
    for text in [
        "tool run",
        "the file has no switches",
        "tool 42",
        "Tool --force",
        "tool 2run --force",
        "tool run! --force",
        "tool run$MODE",
        "tool run, $MODE",
    ] {
        assert_family_absent(text, Family::Command);
    }
    assert_family_found(
        "tool run --force, --quiet",
        Family::Command,
        "tool run --force",
    );
}

#[test]
fn command_candidates_reject_sentence_words_and_keep_the_parameter() {
    for text in [
        "how do i use the tool with --force",
        "comment lancer l'outil avec --force",
    ] {
        assert_family_absent(text, Family::Command);
        assert_family_found(text, Family::Parameter, "--force");
    }
    assert_family_found(
        "How do I force a rerun with ctm run --force?",
        Family::Command,
        "ctm run --force",
    );
}

#[test]
fn backtick_commands_include_each_code_span_and_keep_the_longest_claim() {
    assert_family_found(
        "Use `tool run --force trailing`",
        Family::Command,
        "tool run --force trailing",
    );
    assert_eq!(
        understand("Use `first` then `second`")
            .identifiers
            .into_iter()
            .filter(|identifier| identifier.family == Family::Command)
            .collect::<Vec<_>>(),
        vec![
            Identifier {
                family: Family::Command,
                text: "first".to_owned(),
            },
            Identifier {
                family: Family::Command,
                text: "second".to_owned(),
            },
        ]
    );
}

#[test]
fn earlier_identifier_family_claims_an_overlapping_span() {
    assert_eq!(
        understand("`config.xml`").identifiers,
        vec![Identifier {
            family: Family::Command,
            text: "config.xml".to_owned(),
        }]
    );
}

#[test]
fn earlier_family_priority_wins_over_a_longer_later_family_span() {
    assert_eq!(
        understand("f=ABC123").identifiers,
        vec![Identifier {
            family: Family::ErrorCode,
            text: "ABC123".to_owned(),
        }]
    );
}

#[test]
fn adjacent_identifier_spans_do_not_overlap() {
    assert_eq!(
        understand("$A$B").identifiers,
        vec![
            Identifier {
                family: Family::Parameter,
                text: "$A".to_owned(),
            },
            Identifier {
                family: Family::Parameter,
                text: "$B".to_owned(),
            },
        ]
    );
    assert_eq!(
        understand("${ROOT}v2").identifiers,
        vec![
            Identifier {
                family: Family::Parameter,
                text: "${ROOT}".to_owned(),
            },
            Identifier {
                family: Family::Version,
                text: "v2".to_owned(),
            },
        ]
    );
}

#[test]
fn identifiers_are_unique_and_sorted_by_appearance() {
    let identifiers = understand("--flag ERR-042 service:8443 v2.4 /etc/app.conf").identifiers;
    assert_eq!(
        identifiers,
        vec![
            Identifier {
                family: Family::Parameter,
                text: "--flag".to_owned(),
            },
            Identifier {
                family: Family::ErrorCode,
                text: "ERR-042".to_owned(),
            },
            Identifier {
                family: Family::Port,
                text: "8443".to_owned(),
            },
            Identifier {
                family: Family::Version,
                text: "v2.4".to_owned(),
            },
            Identifier {
                family: Family::Path,
                text: "/etc/app.conf".to_owned(),
            },
        ]
    );
}

#[test]
fn version_intent_uses_the_first_version_and_defaults_to_none() {
    assert_eq!(
        understand("Compare v2.4 with 3.1").version.as_deref(),
        Some("v2.4")
    );
    assert_eq!(understand("No release is requested").version, None);
}

#[test]
fn query_kinds_cover_english_and_french_rules() {
    for (text, expected) in [
        ("This request failed today", QueryKind::Troubleshooting),
        ("Cette tâche échoue", QueryKind::Troubleshooting),
        ("ERR-042", QueryKind::Troubleshooting),
        (
            "What is the difference between these formats?",
            QueryKind::Comparison,
        ),
        (
            "Quelle est la différence entre ces formats?",
            QueryKind::Comparison,
        ),
        ("List all available results", QueryKind::Global),
        ("Combien de résultats?", QueryKind::Global),
        ("How do I configure this route?", QueryKind::Procedure),
        ("Comment installer cet outil?", QueryKind::Procedure),
        ("What are query identifiers?", QueryKind::Concept),
        ("Qu'est-ce que la normalisation?", QueryKind::Concept),
        ("C'est quoi le sens de ce terme?", QueryKind::Concept),
        ("Identifier families in plain text", QueryKind::Lookup),
    ] {
        assert_eq!(understand(text).kind, expected, "query: {text:?}");
    }
}

#[test]
fn query_kind_precedence_uses_the_first_matching_rule() {
    for (text, expected) in [
        (
            concat!(
                "An error explains the difference between all the routes; ",
                "how to install and explain this."
            ),
            QueryKind::Troubleshooting,
        ),
        (
            "The difference between all the routes is clear.",
            QueryKind::Comparison,
        ),
        (
            "List all results and how to configure the route.",
            QueryKind::Global,
        ),
        ("How to explain this query?", QueryKind::Procedure),
    ] {
        assert_eq!(understand(text).kind, expected, "query: {text:?}");
    }
}

#[test]
fn query_kind_matching_is_accent_insensitive_and_uses_whole_words() {
    assert_eq!(
        understand("LA DIFFÉRENCE ENTRE LES FORMATS").kind,
        QueryKind::Comparison
    );
    for text in [
        "errorcode versus2 alloy",
        "alligator configurex meaningfully",
    ] {
        assert_eq!(understand(text).kind, QueryKind::Lookup, "query: {text:?}");
    }
}

#[test]
fn query_kind_folds_typographic_apostrophes_and_extended_latin_once() {
    for (text, expected) in [
        ("Qu’est-ce que le mode strict ?", QueryKind::Concept),
        ("C’est quoi un port ?", QueryKind::Concept),
        ("ÉTAPES pour configurer l’outil", QueryKind::Procedure),
    ] {
        assert_eq!(understand(text).kind, expected, "query: {text:?}");
    }
}

#[test]
fn normalization_keeps_case_collapses_whitespace_and_is_deterministic() {
    let text = "  How\tTo  Configure\nTHE route?  ";
    let understood = understand(text);
    assert_eq!(understood.normalized, "How To Configure THE route?");
    assert_eq!(understood, understand(text));
}

#[test]
fn language_detection_maps_clear_english_and_french_and_short_or_ambiguous_to_unknown() {
    for (text, expected) in [
        (
            "This question needs a stable explanation before it reaches the routes.",
            Language::English,
        ),
        (
            "Cette question nécessite une explication claire avant les routes.",
            Language::French,
        ),
        ("help", Language::Unknown),
        ("Why is this?", Language::English),
        ("Hello there", Language::Unknown),
        ("the this", Language::Unknown),
        ("the this to!", Language::English),
        ("the route et le chemin", Language::Unknown),
        ("12345678901234567890", Language::Unknown),
    ] {
        assert_eq!(understand(text).language, expected, "query: {text:?}");
    }
}
