//! `resolve_expected` gives the ladder's scorer each question's expected
//! sections, and documents expected whole, as `run` resolves them.

use super::run::{
    BACKUPS, NOTES, QUEUES, document, document_id, line, lookup, section_id, suite, three,
};
use crate::eval::{RunError, SectionRef, resolve_expected};
use maestro_canonicalization::{CanonicalizeInput, canonicalize};
use serde_json::json;
use std::convert::Infallible;

/// The pinned revision of the document of `source_ref`.
fn revision_id(source_ref: &str) -> String {
    document(source_ref).unwrap().revision_id
}

#[test]
fn each_question_expects_its_resolved_sections_in_the_suites_order() {
    let resolved = resolve_expected(&three(), lookup()).unwrap();
    assert_eq!(
        resolved,
        [
            vec![SectionRef {
                document_id: document_id(BACKUPS).to_owned(),
                revision_id: Some(revision_id(BACKUPS)),
                chunk_id: None,
                section_id: Some(section_id(BACKUPS, &["Backups", "Retention"], 1).to_owned()),
                span: Some([31, 59]),
                component: None,
            }],
            vec![SectionRef {
                document_id: document_id(QUEUES).to_owned(),
                revision_id: Some(revision_id(QUEUES)),
                chunk_id: None,
                section_id: Some(section_id(QUEUES, &["Queues", "Retries"], 2).to_owned()),
                span: Some([36, 71]),
                component: None,
            }],
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
        [vec![SectionRef {
            document_id: document_id(NOTES).to_owned(),
            revision_id: Some(revision_id(NOTES)),
            chunk_id: None,
            section_id: None,
            span: None,
            component: None,
        }]]
    );
}

#[test]
fn a_nested_list_heading_uses_its_lexical_section_extent() {
    const SOURCE_REF: &str = "corpus-path:lists.md";
    let markdown = concat!(
        "# Root\n\n## Install\n\n- Item\n  ### Note\n  nested text\n  still nested\n",
        "- Another\n\nAfter list prose.\n\n## Next\n\nend\n",
    );
    let mut input = CanonicalizeInput::new(markdown, "lists.md");
    input.metadata.source_reference = Some(SOURCE_REF.to_owned());
    let document = canonicalize(input).unwrap();
    let expected_start = markdown.find("### Note").unwrap();
    let after_list = markdown.find("After list prose.").unwrap();
    let nested = suite(&[line(
        "nested",
        &json!([{
            "source_ref": SOURCE_REF,
            "heading_path": ["Root", "Install", "Note"],
        }]),
    )]);

    let resolved = resolve_expected(&nested, |source_ref| {
        Ok::<_, Infallible>((source_ref == SOURCE_REF).then(|| document.clone()))
    })
    .unwrap();
    let span = resolved[0][0].span.unwrap();

    assert_eq!(span[0], expected_start);
    assert!(span[0] < span[1] && span[1] < after_list);
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

#[test]
fn each_expected_section_keeps_the_component_its_name_gives_in_order() {
    let composed = suite(&[line(
        "composed",
        &json!([
            {
                "source_ref": BACKUPS,
                "heading_path": ["Backups", "Retention"],
                "component": "condition",
            },
            {"source_ref": NOTES, "heading_path": []},
            {
                "source_ref": QUEUES,
                "heading_path": ["Queues", "Retries"],
                "occurrence": 2,
                "component": "action",
            },
        ]),
    )]);

    let resolved = resolve_expected(&composed, lookup()).unwrap();
    let components: Vec<(&str, Option<&str>)> = resolved[0]
        .iter()
        .map(|expected| (expected.document_id.as_str(), expected.component.as_deref()))
        .collect();

    assert_eq!(
        components,
        [
            (document_id(BACKUPS), Some("condition")),
            (document_id(NOTES), None),
            (document_id(QUEUES), Some("action")),
        ]
    );
}
