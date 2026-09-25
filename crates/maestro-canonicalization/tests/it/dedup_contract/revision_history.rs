//! Revisions of one document grouped together keep their own policy,
//! extractor content, locations and run metadata.
use super::batch_inputs::scope;
use maestro_canonicalization::{
    CanonicalDocument, CanonicalizeInput, DedupInput, ExtractorBlock, OriginalLocation, SourceSpan,
    ValidationStatus, WarningPolicy, canonicalize, group_exact,
};
use serde_json::json;
use std::ptr;

#[test]
fn policies_coordinates_and_revision_history_remain_per_occurrence() {
    let text = "# Title\n\nSame content.\n";
    let mut old = sourced_input(text);
    check_a_clean_revision_groups_under_reject(text, &old);
    old.extractor_blocks.push(extractor_block(text));
    let updated = updated_revision(&old);
    let old_doc = canonicalize(old).unwrap();
    let updated_doc = canonicalize(updated).unwrap();
    check_two_revisions_of_one_document(&old_doc, &updated_doc);
    let before = serde_json::to_vec(&(&old_doc, &updated_doc)).unwrap();
    check_each_occurrence_keeps_its_own_revision(text, &old_doc, &updated_doc);
    assert_eq!(
        before,
        serde_json::to_vec(&(&old_doc, &updated_doc)).unwrap()
    );
}

/// An input with a source reference, title, language, extraction record,
/// access policy and run identifier.
fn sourced_input(text: &str) -> CanonicalizeInput<'_> {
    let mut old = CanonicalizeInput::new(text, "stable-source");
    old.metadata.source_reference = Some("urn:test:source".into());
    old.metadata.title = Some("Title".into());
    old.metadata.language = Some("en".into());
    old.metadata.extraction = Some(json!({"converter": "synthetic-test"}));
    old.metadata.access_policy = Some(json!({"readers": ["reader-a"]}));
    old.operational_metadata
        .insert("run_id".into(), json!("run-a"));
    old
}

/// Without extractor content the revision is valid and groups even when
/// warnings are rejected.
fn check_a_clean_revision_groups_under_reject(text: &str, old: &CanonicalizeInput<'_>) {
    let clean = canonicalize(old.clone()).unwrap();
    assert_eq!(clean.validation_status, ValidationStatus::Valid);
    let clean_scope = scope(&[&clean]);
    assert_eq!(
        group_exact(
            &clean_scope,
            &[DedupInput {
                document: &clean,
                markdown: text
            }],
            WarningPolicy::Reject
        )
        .unwrap()
        .occurrences
        .len(),
        1
    );
}

/// Extractor content covering the whole text, with an original page location.
fn extractor_block(text: &str) -> ExtractorBlock {
    ExtractorBlock {
        extractor_id: "extractor-a".into(),
        markdown_spans: vec![SourceSpan {
            start: 0,
            end: text.len(),
        }],
        original_locations: vec![OriginalLocation {
            source_reference: Some("urn:pdf:source".into()),
            page: Some(1),
            locator: Some(json!([1, 2, 3, 4])),
        }],
        structured_content: json!({"kind": "paragraph", "value": "Same content."}),
    }
}

/// The same text under another policy, extraction record, run, extractor and page.
fn updated_revision<'a>(old: &CanonicalizeInput<'a>) -> CanonicalizeInput<'a> {
    let mut updated = old.clone();
    updated.metadata.access_policy = Some(json!({"readers": ["reader-b"]}));
    updated.metadata.extraction = Some(json!({"converter": "synthetic-test-2"}));
    updated
        .operational_metadata
        .insert("run_id".into(), json!("run-b"));
    updated.extractor_blocks[0].extractor_id = "extractor-b".into();
    updated.extractor_blocks[0].original_locations[0].page = Some(2);
    updated
}

/// Both revisions carry only the retained-extractor warning, share the
/// document identity and differ in revision.
fn check_two_revisions_of_one_document(old: &CanonicalDocument, updated: &CanonicalDocument) {
    for doc in [old, updated] {
        assert_eq!(doc.validation_status, ValidationStatus::ValidWithWarnings);
        assert_eq!(
            doc.warnings
                .iter()
                .map(|warning| warning.code.as_str())
                .collect::<Vec<_>>(),
            ["extractor_payload_retained"]
        );
    }
    assert_eq!(old.document_id, updated.document_id);
    assert_ne!(old.revision_id, updated.revision_id);
}

/// Grouped together, each occurrence keeps its own revision's policy,
/// extractor content and run metadata.
fn check_each_occurrence_keeps_its_own_revision(
    text: &str,
    old: &CanonicalDocument,
    updated: &CanonicalDocument,
) {
    let authorization = scope(&[old, updated]);
    let result = group_exact(
        &authorization,
        &[
            DedupInput {
                document: old,
                markdown: text,
            },
            DedupInput {
                document: updated,
                markdown: text,
            },
        ],
        WarningPolicy::Preserve,
    )
    .unwrap();
    assert_eq!(result.groups.len(), 2);
    assert!(
        result
            .groups
            .iter()
            .all(|group| group.occurrence_indices == [0, 1])
    );
    for expected in [old, updated] {
        let occurrence = result
            .occurrences
            .iter()
            .find(|occurrence| occurrence.document.revision_id == expected.revision_id)
            .unwrap();
        assert!(ptr::eq(occurrence.document, expected));
        assert_eq!(occurrence.document.access_policy, expected.access_policy);
        assert_eq!(
            occurrence.document.extractor_blocks,
            expected.extractor_blocks
        );
        assert_eq!(
            occurrence.document.operational_metadata,
            expected.operational_metadata
        );
    }
}
