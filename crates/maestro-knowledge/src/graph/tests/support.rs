//! What the graph tests share: the frozen synthetic defaults table of plan
//! A0, its standalone rule, and sources canonicalized from it or from a
//! variant of it.

use crate::graph::verify::Source;
use maestro_canonicalization::{CanonicalizeInput, canonicalize};
use maestro_kernel::artifact::Digest;
use serde_json::{Map, Value};
use std::{fs, path::PathBuf};

/// The frozen fixture directory, `tests/fixtures/synthetic/graph`.
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/synthetic/graph")
}

/// The frozen Markdown, `defaults.md`.
pub(super) fn markdown() -> String {
    fs::read_to_string(fixtures().join("defaults.md")).unwrap()
}

/// The frozen envelope, `defaults.json`: the rule and its test oracle.
pub(super) fn envelope() -> Value {
    serde_json::from_str(&fs::read_to_string(fixtures().join("defaults.json")).unwrap()).unwrap()
}

/// The standalone frozen rule, as the private rule file has it: the
/// envelope's `rule` object alone.
pub(super) fn rule_text() -> String {
    envelope()["rule"].to_string()
}

/// The frozen rule bound to `markdown`'s digest, with the fields of
/// `changes` replaced or added, as JSON text.
pub(super) fn rule_for(markdown: &str, changes: &Value) -> String {
    let mut rule: Map<String, Value> = serde_json::from_str(&rule_text()).unwrap();
    rule.insert(
        "source_sha256".to_owned(),
        Value::from(Digest::of(markdown.as_bytes()).as_str()),
    );
    for (key, value) in changes.as_object().unwrap() {
        rule.insert(key.clone(), value.clone());
    }
    Value::Object(rule).to_string()
}

/// `markdown`, canonicalized as the import canonicalizes
/// `graph/defaults.md`, as the revision canonicalization names.
pub(super) fn source(markdown: &str) -> Source {
    source_at(markdown, "graph/defaults.md")
}

/// `markdown`, canonicalized as the document `identity`, as the revision
/// canonicalization names.
pub(super) fn source_at(markdown: &str, identity: &str) -> Source {
    let canonical = canonicalize(CanonicalizeInput::new(markdown, identity)).unwrap();
    Source::new(
        canonical.revision_id.clone(),
        canonical,
        markdown.to_owned(),
    )
}

/// The frozen Markdown with its parameter table's body rows replaced by
/// `rows`, each a complete row with its line break.
pub(super) fn with_rows(rows: &str) -> String {
    let markdown = markdown();
    let body = markdown.find("| label").unwrap();
    format!("{}{rows}", &markdown[..body])
}
