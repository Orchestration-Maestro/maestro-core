//! The closed table rule: what it refuses to read, the `DEFAULTS_TO` claims
//! it extracts from the frozen table, the rows it rejects with their reason,
//! and how the subjects it names resolve.

use super::support::{envelope, markdown, rule_for, rule_text, source, source_at, with_rows};
use crate::graph::rules::{Extractor as _, Resolution, TableRule, resolve};
use maestro_kernel::{
    artifact::Digest,
    evidence::Span,
    facts::{EntityName, Literal, LiteralKind, Predicate, Validity},
};
use serde_json::{Value, json};

/// The reason `text` is refused as a rule.
fn refusal(text: &str) -> String {
    TableRule::parse(text).unwrap_err().to_string()
}

/// The frozen rule with the fields of `changes` replaced or added.
fn changed(changes: &Value) -> String {
    rule_for(&markdown(), changes)
}

/// A subject of kind `kind` spelled `name`.
fn named(kind: &str, name: &str) -> EntityName {
    EntityName {
        kind: kind.to_owned(),
        name: name.to_owned(),
    }
}

/// The reasons of the rejections `markdown` gets from the frozen rule bound
/// to it, and how many claims it gets.
fn rejected(markdown: &str) -> (Vec<String>, usize) {
    let rule = TableRule::parse(&rule_for(markdown, &json!({}))).unwrap();
    let extraction = rule.extract(&source(markdown));
    let reasons = extraction
        .rejections
        .into_iter()
        .map(|rejection| rejection.reason)
        .collect();
    (reasons, extraction.claims.len())
}

#[test]
fn the_standalone_frozen_rule_parses() {
    let rule = TableRule::parse(&rule_text()).unwrap();
    assert_eq!(rule.id(), "synthetic-defaults/1");
    assert_eq!(
        rule.source_sha256().as_str(),
        "8cfbf93dbaa5dc25bf9c3a6d88f1b698a19c7c79c6bb832320977e95220766a3"
    );
    assert_eq!(rule.provenance().extractor, "synthetic-defaults/1");
    assert_eq!(rule.provenance().profile, rule.profile());
}

#[test]
fn the_test_envelope_and_its_oracle_are_never_a_rule() {
    let refused = refusal(&envelope().to_string());
    assert!(refused.contains("unknown field `"), "{refused}");
    let mut with_oracle: Value = serde_json::from_str(&rule_text()).unwrap();
    with_oracle["expected"] = envelope()["expected"].clone();
    let refused = refusal(&with_oracle.to_string());
    assert!(refused.contains("unknown field `expected`"), "{refused}");
}

#[test]
fn unknown_and_executable_fields_are_refused() {
    for field in ["script", "expression", "transform", "comment"] {
        let refused = refusal(&changed(&json!({ field: "x => x" })));
        assert!(
            refused.contains(&format!("unknown field `{field}`")),
            "{refused}"
        );
    }
}

#[test]
fn a_duplicate_key_is_refused() {
    let text = rule_text().replacen(
        "\"predicate\":\"DEFAULTS_TO\"",
        "\"predicate\":\"DEFAULTS_TO\",\"predicate\":\"DEFAULTS_TO\"",
        1,
    );
    assert_ne!(text, rule_text());
    let refused = refusal(&text);
    assert!(refused.contains("duplicate field `predicate`"), "{refused}");
}

#[test]
fn a_rule_that_is_not_one_object_is_refused() {
    for text in ["[]", "\"rule\"", "", "{} {}"] {
        assert!(TableRule::parse(text).is_err(), "{text}");
    }
    let rule = rule_text();
    assert!(TableRule::parse(&format!("{rule} {rule}")).is_err());
}

#[test]
fn closed_values_are_refused_when_they_differ() {
    for (changes, reason) in [
        (
            json!({"schema": "maestro-graph-table-rule/2"}),
            "unknown variant",
        ),
        (json!({"predicate": "DEPENDS_ON"}), "unknown variant"),
        (json!({"source_sha256": "8CFB"}), "SHA-256"),
        (json!({"id": ""}), "id is empty"),
        (json!({"subject_kind": ""}), "subject kind"),
        (json!({"subject_kind": "Parameter kind"}), "subject kind"),
        (json!({"heading_path": []}), "heading path"),
        (
            json!({"heading_path": ["Lantern controller", ""]}),
            "heading path",
        ),
        (json!({"columns": []}), "no column"),
        (json!({"columns": ["Parameter", "Type", "Type"]}), "twice"),
        (
            json!({"columns": ["Parameter", "", "Default"]}),
            "empty column",
        ),
        (json!({"lexeme_column": "Value"}), "not a declared column"),
        (json!({"type_column": "Parameter"}), "distinct"),
        (json!({"lexeme_column": "Type"}), "distinct"),
        (json!({"subject_column": "Default"}), "distinct"),
    ] {
        let refused = refusal(&changed(&changes));
        assert!(refused.contains(reason), "{changes}: {refused}");
    }
}

#[test]
fn the_frozen_table_gives_the_four_oracle_defaults() {
    let markdown = markdown();
    let source = source(&markdown);
    let rule = TableRule::parse(&rule_text()).unwrap();
    let extraction = rule.extract(&source);
    assert_eq!(extraction.rejections, []);
    let expected = envelope()["expected"].as_array().unwrap().clone();
    assert_eq!(extraction.claims.len(), expected.len());
    for (claim, expected) in extraction.claims.iter().zip(&expected) {
        assert_eq!(
            claim.subject,
            named("Parameter", expected["subject"].as_str().unwrap())
        );
        assert_eq!(claim.predicate, Predicate::DefaultsTo);
        let kind = LiteralKind::parse(expected["object"]["type"].as_str().unwrap()).unwrap();
        let lexeme = expected["object"]["lexeme"].as_str().unwrap().to_owned();
        assert_eq!(claim.object, Literal { kind, lexeme });
        assert!(claim.conditions.is_empty());
        assert_eq!(
            (&claim.version, &claim.world),
            (&Validity::Unknown, &Validity::Unknown)
        );
        assert_eq!(claim.provenance, rule.provenance());
        let [support] = claim.supports.as_slice() else {
            panic!("one support: {claim:?}");
        };
        let span = Span {
            start: usize::try_from(expected["span"][0].as_u64().unwrap()).unwrap(),
            end: usize::try_from(expected["span"][1].as_u64().unwrap()).unwrap(),
        };
        assert_eq!(support.span, span);
        assert_eq!(support.revision_id, source.revision_id());
        let quote = &markdown.as_bytes()[span.start..span.end];
        assert_eq!(quote, expected["quote"].as_str().unwrap().as_bytes());
        assert_eq!(support.quote_digest, Digest::of(quote));
        assert_eq!(support.quote_digest.as_str(), expected["quote_sha256"]);
    }
}

#[test]
fn defaults_cite_their_canonical_row_blocks() {
    let markdown = markdown();
    let source = source(&markdown);
    let extraction = TableRule::parse(&rule_text()).unwrap().extract(&source);
    let rows: Vec<&str> = source
        .canonical()
        .blocks
        .iter()
        .filter(|block| block.block_type == maestro_canonicalization::BlockType::TableRow)
        .map(|block| block.block_id.as_str())
        .collect();
    let cited: Vec<&str> = extraction
        .claims
        .iter()
        .map(|claim| claim.supports[0].block_id.as_str())
        .collect();
    assert_eq!(cited, rows);
}

#[test]
fn literals_keep_their_source_lexemes_and_become_no_entity() {
    let extraction = TableRule::parse(&rule_text())
        .unwrap()
        .extract(&source(&markdown()));
    let objects: Vec<(LiteralKind, &str)> = extraction
        .claims
        .iter()
        .map(|claim| (claim.object.kind, claim.object.lexeme.as_str()))
        .collect();
    assert_eq!(
        objects,
        [
            (LiteralKind::Text, "café"),
            (LiteralKind::Boolean, "true"),
            (LiteralKind::Integer, "3"),
            (LiteralKind::Decimal, "0.50"),
        ]
    );
    let subjects: Vec<EntityName> = extraction
        .claims
        .iter()
        .map(|claim| claim.subject.clone())
        .collect();
    let entities: Vec<EntityName> = resolve(&subjects)
        .into_iter()
        .map(|resolution| resolution.subject)
        .collect();
    assert_eq!(
        entities,
        ["enabled", "label", "ratio", "retries"].map(|name| named("Parameter", name))
    );
}

#[test]
fn a_changed_source_digest_extracts_nothing() {
    let markdown = markdown();
    let other = markdown.replace("café", "cafe");
    let rule = TableRule::parse(&rule_text()).unwrap();
    let extraction = rule.extract(&source(&other));
    assert!(extraction.claims.is_empty());
    let [rejection] = extraction.rejections.as_slice() else {
        panic!("one rejection: {extraction:?}");
    };
    assert!(rejection.reason.contains("source digest"), "{rejection:?}");
    assert_eq!(rejection.block_id, None);
}

#[test]
fn a_misleading_heading_selects_no_table() {
    for heading in [
        "## Parameters (legacy)",
        "## parameters",
        "## Parameters\n\n### Deprecated",
    ] {
        let markdown = markdown().replace("## Parameters", heading);
        let (reasons, claims) = rejected(&markdown);
        assert_eq!(claims, 0, "{heading}");
        assert_eq!(reasons.len(), 1, "{heading}: {reasons:?}");
        assert!(reasons[0].contains("no table under"), "{reasons:?}");
    }
}

#[test]
fn two_tables_under_the_heading_are_ambiguous() {
    let markdown = format!(
        "{}\n{}",
        markdown(),
        &markdown()[markdown().find("| Parameter").unwrap()..]
    );
    let (reasons, claims) = rejected(&markdown);
    assert_eq!(claims, 0);
    assert_eq!(reasons.len(), 2, "{reasons:?}");
    assert!(
        reasons.iter().all(|reason| reason.contains("2 tables")),
        "{reasons:?}"
    );
}

#[test]
fn a_table_with_other_columns_is_rejected() {
    let markdown = markdown().replace(
        "| Parameter | Type | Default |",
        "| Parameter | Kind | Default |",
    );
    let (reasons, claims) = rejected(&markdown);
    assert_eq!(claims, 0);
    assert_eq!(reasons.len(), 1, "{reasons:?}");
    assert!(reasons[0].contains("columns"), "{reasons:?}");
}

#[test]
fn a_bad_row_is_rejected_alone_with_its_reason() {
    for (row, reason) in [
        ("| ratio | float | 0.50 |\n", "not a literal type"),
        ("| ratio | integer | 0.50 |\n", "does not read as integer"),
        ("| ratio | boolean | yes |\n", "does not read as boolean"),
        ("| ratio | decimal |  |\n", "empty"),
        ("|  | decimal | 0.50 |\n", "empty"),
        ("| ratio | decimal | **0.50** |\n", "not the source"),
        ("| ratio | decimal | 0\\.50 |\n", "not the source"),
        ("| ratio | decimal | 0.50 | extra |\n", "outside its cells"),
        ("| ratio | decimal |\n", "is empty"),
    ] {
        let markdown = with_rows(&format!("| retries | integer | 3 |\n{row}"));
        let (reasons, claims) = rejected(&markdown);
        assert_eq!(claims, 1, "{row}: {reasons:?}");
        assert_eq!(reasons.len(), 1, "{row}: {reasons:?}");
        assert!(reasons[0].contains(reason), "{row}: {reasons:?}");
    }
}

#[test]
fn a_subject_in_two_rows_is_ambiguous() {
    let markdown = with_rows(
        "| retries | integer | 3 |\n| retries | integer | 5 |\n| ratio | decimal | 0.50 |\n",
    );
    let (reasons, claims) = rejected(&markdown);
    assert_eq!(claims, 1);
    assert_eq!(reasons.len(), 2, "{reasons:?}");
    assert!(
        reasons.iter().all(|reason| reason.contains("2 rows")),
        "{reasons:?}"
    );
}

#[test]
fn a_rule_change_changes_its_profile() {
    let frozen = TableRule::parse(&rule_text()).unwrap();
    assert_eq!(
        frozen.profile(),
        TableRule::parse(&rule_text()).unwrap().profile()
    );
    let spaced = serde_json::to_string_pretty(&envelope()["rule"]).unwrap();
    assert_eq!(
        frozen.profile(),
        TableRule::parse(&spaced).unwrap().profile()
    );
    for changes in [
        json!({"id": "synthetic-defaults/2"}),
        json!({"subject_kind": "Setting"}),
        json!({"heading_path": ["Lantern controller"]}),
        json!({"columns": ["Parameter", "Default", "Type"]}),
        json!({"source_sha256": Digest::of(b"other").as_str()}),
    ] {
        let other = TableRule::parse(&changed(&changes)).unwrap();
        assert_ne!(other.profile(), frozen.profile(), "{changes}");
    }
}

#[test]
fn the_same_spelling_and_kind_resolves_to_one_entity_across_documents() {
    let resolved = resolve(&[
        named("Parameter", "label"),
        named("Parameter", "retries"),
        named("Parameter", "label"),
    ]);
    assert_eq!(
        resolved,
        [
            Resolution {
                subject: named("Parameter", "label"),
                normalized: "label".to_owned(),
                colliding: vec![],
            },
            Resolution {
                subject: named("Parameter", "retries"),
                normalized: "retries".to_owned(),
                colliding: vec![],
            },
        ]
    );
}

#[test]
fn colliding_spellings_or_kinds_stay_ambiguous() {
    let resolved = resolve(&[
        named("Parameter", "Café"),
        named("Parameter", "cafe"),
        named("Setting", "cafe"),
    ]);
    assert_eq!(
        resolved,
        [
            Resolution {
                subject: named("Parameter", "Café"),
                normalized: "cafe".to_owned(),
                colliding: vec![named("Parameter", "cafe"), named("Setting", "cafe")],
            },
            Resolution {
                subject: named("Parameter", "cafe"),
                normalized: "cafe".to_owned(),
                colliding: vec![named("Parameter", "Café"), named("Setting", "cafe")],
            },
            Resolution {
                subject: named("Setting", "cafe"),
                normalized: "cafe".to_owned(),
                colliding: vec![named("Parameter", "Café"), named("Parameter", "cafe")],
            },
        ]
    );
}

#[test]
fn a_subject_kind_may_hold_digits_and_underscores() {
    let rule = TableRule::parse(&changed(&json!({"subject_kind": "Lantern_parameter2"}))).unwrap();
    let extraction = rule.extract(&source(&markdown()));
    assert_eq!(extraction.claims.len(), 4);
    assert!(
        extraction
            .claims
            .iter()
            .all(|claim| claim.subject.kind == "Lantern_parameter2")
    );
}

#[test]
fn a_table_without_outer_pipes_gives_the_same_defaults() {
    let markdown = markdown()
        .lines()
        .map(|line| line.trim_start_matches("| ").trim_end_matches(" |"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert!(markdown.contains("\nlabel | text | café\n"), "{markdown}");
    let rule = TableRule::parse(&rule_for(&markdown, &json!({}))).unwrap();
    let extraction = rule.extract(&source(&markdown));
    assert_eq!(extraction.rejections, []);
    let lexemes: Vec<&str> = extraction
        .claims
        .iter()
        .map(|claim| claim.object.lexeme.as_str())
        .collect();
    assert_eq!(lexemes, ["café", "true", "3", "0.50"]);
}

#[test]
fn names_normalize_without_accents_case_or_repeated_spaces() {
    let resolved = resolve(&[
        named("Parameter", "Retry  Count"),
        named("Parameter", "retry count"),
    ]);
    assert_eq!(resolved.len(), 2);
    assert!(
        resolved
            .iter()
            .all(|resolution| resolution.normalized == "retry count")
    );
    assert!(
        resolved
            .iter()
            .all(|resolution| resolution.colliding.len() == 1)
    );
}

#[test]
fn a_subject_with_a_bad_row_is_refused_whole() {
    for bad in [
        "| retries | integer | 3 (Windows) |\n",
        "| **retries** | integer | 5 |\n",
    ] {
        let markdown = with_rows(&format!(
            "| retries | integer | 3 |\n{bad}| ratio | decimal | 0.50 |\n"
        ));
        let rule = TableRule::parse(&rule_for(&markdown, &json!({}))).unwrap();
        let extraction = rule.extract(&source(&markdown));
        let subjects: Vec<&str> = extraction
            .claims
            .iter()
            .map(|claim| claim.subject.name.as_str())
            .collect();
        assert_eq!(subjects, ["ratio"], "{bad}");
        let reasons: Vec<&str> = extraction
            .rejections
            .iter()
            .map(|rejection| rejection.reason.as_str())
            .collect();
        assert_eq!(reasons.len(), 2, "{bad}: {reasons:?}");
        assert!(
            reasons[0].contains("ambiguous: it has 2 rows"),
            "{reasons:?}"
        );
        assert!(!reasons[1].contains("ambiguous"), "{reasons:?}");
    }
}

#[test]
fn a_role_binding_change_changes_the_profile() {
    let frozen = TableRule::parse(&rule_text()).unwrap();
    let swapped = changed(&json!({"type_column": "Default", "lexeme_column": "Type"}));
    assert_ne!(
        TableRule::parse(&swapped).unwrap().profile(),
        frozen.profile()
    );
}

#[test]
fn the_same_rows_in_two_documents_resolve_to_four_entities() {
    let markdown = markdown();
    let rule = TableRule::parse(&rule_text()).unwrap();
    let mut subjects = Vec::new();
    for identity in ["graph/defaults.md", "graph/copy.md"] {
        let extraction = rule.extract(&source_at(&markdown, identity));
        assert_eq!(extraction.rejections, [], "{identity}");
        subjects.extend(extraction.claims.into_iter().map(|claim| claim.subject));
    }
    assert_eq!(subjects.len(), 8);
    let resolved = resolve(&subjects);
    assert_eq!(resolved.len(), 4);
    assert!(
        resolved
            .iter()
            .all(|resolution| resolution.colliding.is_empty())
    );
}

#[test]
fn a_header_with_markup_rejects_the_table() {
    let markdown = markdown().replace("| Parameter |", "| **Parameter** |");
    let source = source(&markdown);
    let rule = TableRule::parse(&rule_for(&markdown, &json!({}))).unwrap();
    let extraction = rule.extract(&source);
    assert!(extraction.claims.is_empty());
    let [rejection] = extraction.rejections.as_slice() else {
        panic!("one rejection: {extraction:?}");
    };
    let table = source
        .canonical()
        .blocks
        .iter()
        .find(|block| block.block_type == maestro_canonicalization::BlockType::Table)
        .unwrap();
    assert_eq!(rejection.block_id.as_deref(), Some(table.block_id.as_str()));
    assert!(rejection.reason.contains("not the source"), "{rejection:?}");
}
