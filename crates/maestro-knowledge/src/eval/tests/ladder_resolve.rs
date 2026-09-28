//! `resolve_expected` gives the ladder's scorer each question's expected
//! sections, and documents expected whole, as `run` resolves them.

use super::run::{BACKUPS, NOTES, QUEUES, document_id, line, lookup, section_id, suite, three};
use crate::eval::{RunError, SectionRef, resolve_expected};
use serde_json::json;

#[test]
fn each_question_expects_its_resolved_sections_in_the_suites_order() {
    let resolved = resolve_expected(&three(), lookup()).unwrap();
    assert_eq!(
        resolved,
        [
            vec![SectionRef::section(
                document_id(BACKUPS),
                section_id(BACKUPS, &["Backups", "Retention"], 1)
            )],
            vec![SectionRef::section(
                document_id(QUEUES),
                section_id(QUEUES, &["Queues", "Retries"], 2)
            )],
            vec![],
        ]
    );
}

#[test]
fn a_document_without_sections_is_expected_whole() {
    let notes = suite(&[line(
        "notes",
        &json!([{"source_ref": NOTES, "heading_path": []}]),
    )]);
    assert_eq!(
        resolve_expected(&notes, lookup()).unwrap(),
        [vec![SectionRef::document(document_id(NOTES))]]
    );
}

#[test]
fn a_name_its_document_lacks_is_refused() {
    let unknown = suite(&[line(
        "missing",
        &json!([{"source_ref": "corpus-path:missing.md", "heading_path": ["Missing"]}]),
    )]);
    assert!(matches!(
        resolve_expected(&unknown, lookup()),
        Err(RunError::NoDocument { question, .. }) if question == "missing"
    ));
}
