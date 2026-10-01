//! G01's synthetic pilot contract, not the G03 production rule engine.
//! Checks frozen source bytes, canonical table anchors, expected defaults and
//! the requirement ownership table and approved manifest handoff before later
//! graph tasks consume them. Document checks are not runtime qualification.
#![cfg(test)]

use maestro_canonicalization::{Block, BlockType, CanonicalizeInput, SourceSpan, canonicalize};
use maestro_kernel::artifact::Digest;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, mem, path::PathBuf};

/// Original source bytes frozen in plan A0; changes need an explicit re-freeze.
const SOURCE_SHA256: &str = "8cfbf93dbaa5dc25bf9c3a6d88f1b698a19c7c79c6bb832320977e95220766a3";
/// Complete fixture bytes frozen alongside the source in plan A0.
const FIXTURE_SHA256: &str = "3443fe932c03e9042e08514c33b4034fc18cf302b2e296cdfa44531adcfefbf9";

/// Closed fixture envelope; expected claims are an oracle, not rule inputs.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    /// The one data-only table rule frozen for the pilot.
    rule: Rule,
    /// Ordered source-backed defaults, separate from extraction configuration.
    expected: Vec<DefaultClaim>,
}

/// The pilot selects exactly one table; it has no executable fields.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    /// Version of the closed table-rule contract.
    schema: String,
    /// Synthetic extraction rule identity.
    id: String,
    /// Digest of the complete original Markdown, bound without the test oracle.
    source_sha256: Digest,
    /// Exact canonical heading path, not a substring match.
    heading_path: Vec<String>,
    /// Exact ordered table headers.
    columns: Vec<String>,
    /// Header containing the subject's original spelling.
    subject_column: String,
    /// Header containing the literal's declared type.
    type_column: String,
    /// Header containing the unchanged default lexeme.
    lexeme_column: String,
    /// Explicit subject kind, never inferred from a default value.
    subject_kind: String,
    /// The sole pilot predicate.
    predicate: String,
}

/// One literal-valued claim and its original row anchor.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultClaim {
    /// Exact source spelling of the parameter.
    subject: String,
    /// Explicit source type and unchanged default lexeme.
    object: Literal,
    /// Nonempty half-open UTF-8 source offsets of the complete table row.
    span: [usize; 2],
    /// Original row, including Markdown syntax and its newline.
    quote: String,
    /// Digest of that exact row.
    quote_sha256: Digest,
}

/// No numeric conversion can rewrite a decimal such as `0.50`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Literal {
    /// One of the four source-declared literal types.
    #[serde(rename = "type")]
    kind: LiteralKind,
    /// Original cell text, without floating-point conversion.
    lexeme: String,
}

/// Closed literal family; none of these objects becomes an entity node.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LiteralKind {
    /// Text value, including non-ASCII characters.
    Text,
    /// Boolean source lexeme.
    Boolean,
    /// Integer source lexeme.
    Integer,
    /// Decimal source lexeme.
    Decimal,
}

impl LiteralKind {
    /// The spelling required in the table's explicit Type column.
    fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Decimal => "decimal",
        }
    }
}

/// Workspace-relative fixtures also work on native Windows and macOS.
pub(super) fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Read at test time so a missing fixture fails the Red check, not compilation.
fn fixture() -> (String, Fixture) {
    let root = workspace().join("tests/fixtures/synthetic/graph");
    let source = fs::read_to_string(root.join("defaults.md")).unwrap();
    let json = fs::read_to_string(root.join("defaults.json")).unwrap();
    (source, serde_json::from_str(&json).unwrap())
}

/// Direct canonical cells in their source order, without a second Markdown parser.
fn cells<'a>(blocks: &'a [Block], row: &Block) -> Vec<&'a str> {
    blocks
        .iter()
        .filter(|block| block.parent_block_id.as_ref() == Some(&row.block_id))
        .map(|block| block.retrieval_text.as_str())
        .collect()
}

/// Resolve cell roles by their declared headers, never by an implicit order.
fn bound_cells<'a>(
    blocks: &'a [Block],
    row: &Block,
    rule: &Rule,
) -> Result<Vec<&'a str>, &'static str> {
    let cells = cells(blocks, row);
    [&rule.subject_column, &rule.type_column, &rule.lexeme_column]
        .into_iter()
        .map(|column| {
            let index = rule
                .columns
                .iter()
                .position(|name| name == column)
                .ok_or("column binding")?;
            cells.get(index).copied().ok_or("column binding")
        })
        .collect()
}

/// Verify this fixture's contract only; G02/G03 own claim admission/extraction.
fn check_fixture(source: &str, fixture: &Fixture) -> Result<(), &'static str> {
    if Digest::of(source.as_bytes()) != fixture.rule.source_sha256 {
        return Err("source digest");
    }
    let rule = &fixture.rule;
    if rule.schema != "maestro-graph-table-rule/1"
        || rule.id != "synthetic-defaults/1"
        || rule.subject_kind != "Parameter"
        || rule.predicate != "DEFAULTS_TO"
        || rule.columns != ["Parameter", "Type", "Default"]
    {
        return Err("rule");
    }
    let document = canonicalize(CanonicalizeInput::new(source, "graph/defaults.md"))
        .map_err(|_| "canonical source")?;
    let tables: Vec<_> = document
        .blocks
        .iter()
        .filter(|block| block.block_type == BlockType::Table)
        .collect();
    let [table] = tables.as_slice() else {
        return Err("one table");
    };
    if table.heading_path != rule.heading_path {
        return Err("heading path");
    }
    let rows: Vec<_> = document
        .blocks
        .iter()
        .filter(|block| block.parent_block_id.as_ref() == Some(&table.block_id))
        .collect();
    let Some((header, rows)) = rows.split_first() else {
        return Err("table header");
    };
    if header.block_type != BlockType::TableHead || cells(&document.blocks, header) != rule.columns
    {
        return Err("table header");
    }
    if rows.len() != fixture.expected.len() || rows.is_empty() {
        return Err("default count");
    }
    for (row, expected) in rows.iter().zip(&fixture.expected) {
        check_default(source, row, expected)?;
        if bound_cells(&document.blocks, row, rule)?
            != [
                expected.subject.as_str(),
                expected.object.kind.name(),
                expected.object.lexeme.as_str(),
            ]
        {
            return Err("default value");
        }
    }
    Ok(())
}

/// An exact quote must also be the canonical row, not another matching slice.
fn check_default(source: &str, row: &Block, claim: &DefaultClaim) -> Result<(), &'static str> {
    let [start, end] = claim.span;
    let span = SourceSpan { start, end };
    if start == end || !span.is_valid(source) {
        return Err("UTF-8 span");
    }
    if row.block_type != BlockType::TableRow || row.source_spans != [span] {
        return Err("row anchor");
    }
    let quote = &source[start..end];
    if quote != claim.quote || Digest::of(quote.as_bytes()) != claim.quote_sha256 {
        return Err("quote digest");
    }
    Ok(())
}

#[test]
fn frozen_file_digests_match_the_reviewed_plan() {
    let root = workspace();
    let plan = fs::read_to_string(root.join("specs/002-knowledge-graph/plan.md")).unwrap();
    for (file, digest) in [
        ("defaults.md", SOURCE_SHA256),
        ("defaults.json", FIXTURE_SHA256),
    ] {
        let bytes = fs::read(root.join("tests/fixtures/synthetic/graph").join(file)).unwrap();
        assert_eq!(Digest::of(&bytes).as_str(), digest, "{file}");
        assert!(plan.contains(digest), "plan A0 must freeze {file}");
    }
}

#[test]
fn standalone_rule_binds_source_and_explicit_column_roles_without_an_oracle() {
    let text = fs::read_to_string(workspace().join("tests/fixtures/synthetic/graph/defaults.json"))
        .unwrap();
    let envelope: Value = serde_json::from_str(&text).unwrap();
    let rule = &envelope["rule"];
    assert_eq!(rule["source_sha256"], SOURCE_SHA256);
    assert_eq!(rule["id"], "synthetic-defaults/1");
    for (role, column) in [
        ("subject_column", "Parameter"),
        ("type_column", "Type"),
        ("lexeme_column", "Default"),
    ] {
        assert_eq!(rule[role], column);
    }
    let parsed: Rule = serde_json::from_str(&rule.to_string()).unwrap();
    assert_eq!(parsed.id, "synthetic-defaults/1");
    assert!(serde_json::from_str::<Rule>(&text).is_err());
    for key in ["unknown", "script", "expected"] {
        let mut changed = rule.clone();
        changed[key] = json!("not executable");
        assert!(
            serde_json::from_str::<Rule>(&changed.to_string()).is_err(),
            "{key}"
        );
    }
    let repeated_id = rule.to_string().replacen(
        "\"id\":\"synthetic-defaults/1\"",
        "\"id\":\"synthetic-defaults/1\",\"id\":\"duplicate\"",
        1,
    );
    assert!(serde_json::from_str::<Rule>(&repeated_id).is_err());
}

#[test]
fn frozen_defaults_match_original_rows_and_explicit_literal_types() {
    let (source, fixture) = fixture();
    assert_eq!(check_fixture(&source, &fixture), Ok(()));
    let defaults: Vec<_> = fixture
        .expected
        .iter()
        .map(|claim| {
            (
                claim.subject.as_str(),
                claim.object.kind.name(),
                claim.object.lexeme.as_str(),
            )
        })
        .collect();
    assert_eq!(
        defaults,
        [
            ("label", "text", "café"),
            ("enabled", "boolean", "true"),
            ("retries", "integer", "3"),
            ("ratio", "decimal", "0.50"),
        ]
    );
}

#[test]
fn changed_source_or_quote_digest_is_rejected() {
    let (source, fixture) = fixture();
    assert_eq!(
        check_fixture(&format!("{source}\n"), &fixture),
        Err("source digest")
    );
    let mut changed = fixture.clone();
    changed.rule.source_sha256 = Digest::of(b"different source");
    assert_eq!(check_fixture(&source, &changed), Err("source digest"));
    let mut changed = fixture.clone();
    changed.expected[0].quote_sha256 = Digest::of(b"different quote");
    assert_eq!(check_fixture(&source, &changed), Err("quote digest"));
    let mut changed = fixture;
    changed.expected[0].quote = "invented quote".to_owned();
    assert_eq!(check_fixture(&source, &changed), Err("quote digest"));
}

#[test]
fn empty_reversed_out_of_range_and_split_utf8_spans_are_rejected() {
    let (source, fixture) = fixture();
    let inside = source.find('é').unwrap() + 1;
    for span in [
        [0, 0],
        [2, 1],
        [0, source.len() + 1],
        [inside, source.len()],
        [0, inside],
    ] {
        let mut changed = fixture.clone();
        changed.expected[0].span = span;
        assert_eq!(
            check_fixture(&source, &changed),
            Err("UTF-8 span"),
            "{span:?}"
        );
    }
    let mut changed = fixture;
    changed.expected[0].span = [0, 1];
    assert_eq!(check_fixture(&source, &changed), Err("row anchor"));
}

#[test]
fn unknown_or_swapped_column_roles_are_rejected() {
    let (source, fixture) = fixture();
    for role in ["subject_column", "type_column", "lexeme_column"] {
        let text =
            fs::read_to_string(workspace().join("tests/fixtures/synthetic/graph/defaults.json"))
                .unwrap();
        let mut changed: Value = serde_json::from_str(&text).unwrap();
        changed["rule"][role] = json!("Unknown");
        let changed: Fixture = serde_json::from_value(changed).unwrap();
        assert_eq!(
            check_fixture(&source, &changed),
            Err("column binding"),
            "{role}"
        );
    }
    let mut changed = fixture;
    mem::swap(
        &mut changed.rule.subject_column,
        &mut changed.rule.type_column,
    );
    assert_eq!(check_fixture(&source, &changed), Err("default value"));
}

#[test]
fn wrong_defaults_types_and_missing_rows_are_rejected() {
    let (source, fixture) = fixture();
    let mut changed = fixture.clone();
    changed.expected[3].object.lexeme = "0.5".to_owned();
    assert_eq!(check_fixture(&source, &changed), Err("default value"));
    let mut changed = fixture.clone();
    changed.expected[1].object.kind = LiteralKind::Text;
    assert_eq!(check_fixture(&source, &changed), Err("default value"));
    let mut changed = fixture;
    changed.expected.pop();
    assert_eq!(check_fixture(&source, &changed), Err("default count"));
}

/// Requirement tokens, including wrapped Acceptance lines, in source order.
fn requirements(text: &str) -> impl Iterator<Item = &str> {
    text.split(|character: char| !character.is_ascii_alphanumeric() && character != '-')
        .filter(|word| word.starts_with("FR-S2-") || word.starts_with("SC-S2-"))
}

/// Recognize only task headings (`GNN [US…]`), not headings such as Graph notes.
fn task_id(section: &str) -> Option<&str> {
    let (task, story) = section.strip_prefix('G')?.split_once(' ')?;
    (task.len() == 2 && task.bytes().all(|byte| byte.is_ascii_digit()) && story.starts_with("[US"))
        .then_some(task)
}

/// Compare the complete map, including duplicate rows and task ownership order.
fn check_coverage(tasks: &str) -> Result<(), &'static str> {
    let mut expected = BTreeMap::<&str, Vec<&str>>::new();
    for section in tasks.split("\n### ").skip(1) {
        let Some(task) = task_id(section) else {
            continue;
        };
        let acceptance = section
            .split_once("**Acceptance:**")
            .ok_or("Acceptance line")?
            .1
            .split("\n\n")
            .next()
            .ok_or("Acceptance body")?;
        for requirement in requirements(acceptance) {
            expected.entry(requirement).or_default().push(task);
        }
    }
    let mut actual = BTreeMap::new();
    for line in tasks
        .lines()
        .filter(|line| line.starts_with("| FR-S2-") || line.starts_with("| SC-S2-"))
    {
        let mut columns = line.split('|').skip(1).map(str::trim);
        let requirement = columns.next().ok_or("requirement column")?;
        let owners = columns.next().ok_or("owners column")?;
        let owners: Vec<_> = owners
            .split(", ")
            .map(|owner| owner.strip_prefix('G').ok_or("owner ID"))
            .collect::<Result<_, _>>()?;
        if actual.insert(requirement, owners).is_some() {
            return Err("duplicate requirement row");
        }
    }
    if expected.is_empty() || expected != actual {
        return Err("requirement map differs from Acceptance lines");
    }
    Ok(())
}

#[test]
fn requirement_coverage_equals_every_task_acceptance_line() {
    let tasks = fs::read_to_string(workspace().join("specs/002-knowledge-graph/tasks.md")).unwrap();
    assert_eq!(check_coverage(&tasks), Ok(()));
    let row = "| FR-S2-004 | G01, G03 |";
    assert!(tasks.contains(row));
    for wrong in ["", "| FR-S2-004 | G01 |", "| FR-S2-004 | G01, G03, G25 |"] {
        assert!(
            check_coverage(&tasks.replace(row, wrong)).is_err(),
            "{wrong}"
        );
    }
    assert_eq!(
        check_coverage(&format!("{tasks}\n{row}\n")),
        Err("duplicate requirement row")
    );
    assert!(check_coverage(&format!("{tasks}\n| FR-S2-999 | G01 |\n")).is_err());
    assert_eq!(
        check_coverage(&format!("{tasks}\n### Graph notes\n\nNot a task.\n")),
        Ok(())
    );
}
