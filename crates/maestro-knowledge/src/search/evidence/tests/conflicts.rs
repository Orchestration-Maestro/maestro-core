use super::super::conflicts::detect::{ConflictContext, ConflictSource, detect_conflicts};
use maestro_canonicalization::{CanonicalDocument, CanonicalizeInput, canonicalize};
use maestro_kernel::evidence::Span;
use std::{collections::BTreeSet, fmt::Write as _};

pub(super) fn document(markdown: &str, identity_key: &str) -> CanonicalDocument {
    canonicalize(CanonicalizeInput::new(markdown, identity_key)).unwrap()
}

pub(super) fn source<'a>(
    candidate_index: usize,
    document: &'a CanonicalDocument,
    markdown: &'a str,
    near_group_ids: &'a BTreeSet<String>,
    context: &'a ConflictContext,
) -> ConflictSource<'a> {
    ConflictSource {
        candidate_index,
        document_id: &document.document_id,
        revision_id: &document.revision_id,
        near_group_ids,
        context,
        proposed_extent: Span {
            start: 0,
            end: markdown.len(),
        },
        document,
        markdown,
    }
}

pub(super) fn table(entity: &str, attribute: &str, value: &str) -> String {
    format!(
        concat!(
            "# Guide\n\n| Entity | Attribute | Value |\n",
            "| --- | --- | --- |\n| {entity} | {attribute} | {value} |\n"
        ),
        entity = entity,
        attribute = attribute,
        value = value
    )
}

fn parameter_table(value: &str) -> String {
    format!("# Guide\n\n| Parameter | Default |\n| --- | --- |\n| Connection | {value} |\n")
}

fn multi_table(values: &[&str]) -> String {
    let mut markdown =
        String::from("# Guide\n\n| Entity | Attribute | Value |\n| --- | --- | --- |\n");
    for value in values {
        let _ = writeln!(markdown, "| Agent | Port | {value} |");
    }
    markdown
}

fn assert_invalid_source(source: ConflictSource<'_>) {
    assert_eq!(
        detect_conflicts(&[source]).unwrap_err(),
        "conflict source identity or extent is invalid"
    );
}

#[test]
fn byte_exact_values_conflict_only_in_a_manifest_allowed_correspondence_family() {
    let markdown_a = table("Agent", "Port", "7005");
    let markdown_b = table("Agent", "Port", "7006");
    let document_a = document(&markdown_a, "doc-a.md");
    let document_b = document(&markdown_b, "doc-b.md");
    let context = ConflictContext::default();
    let group = BTreeSet::from(["near-group".to_owned()]);
    let findings = detect_conflicts(&[
        source(0, &document_a, &markdown_a, &group, &context),
        source(1, &document_b, &markdown_b, &group, &context),
    ])
    .unwrap();

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].entity, "Agent");
    assert_eq!(findings[0].attribute, "Port");
    assert_eq!(
        findings[0].values,
        BTreeSet::from(["7005".to_owned(), "7006".to_owned()])
    );
    assert_eq!(findings[0].candidate_indices, BTreeSet::from([0, 1]));
    assert!(findings[0].table_spans.contains_key(&0));
    assert!(findings[0].table_spans.contains_key(&1));
}

#[test]
fn same_revision_overlapping_candidates_share_one_row_observation() {
    let markdown_a = parameter_table("first  value");
    let markdown_b = parameter_table("second  value");
    let document_a = document(&markdown_a, "doc-a.md");
    let document_b = document(&markdown_b, "doc-b.md");
    let context = ConflictContext::default();
    let group = BTreeSet::from(["near-group".to_owned()]);
    let sources = [
        source(0, &document_a, &markdown_a, &group, &context),
        source(1, &document_a, &markdown_a, &group, &context),
        source(2, &document_b, &markdown_b, &group, &context),
    ];
    let findings = detect_conflicts(&sources).unwrap();

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].entity, "Connection");
    assert_eq!(findings[0].attribute, "default");
    assert_eq!(
        findings[0].values,
        BTreeSet::from(["first  value".to_owned(), "second  value".to_owned()])
    );
    assert_eq!(findings[0].candidate_indices, BTreeSet::from([0, 1, 2]));
}

#[test]
fn same_document_revisions_match_without_a_near_group() {
    let markdown_a = table("Agent", "Port", "7005");
    let markdown_b = table("Agent", "Port", "7006");
    let document_a = document(&markdown_a, "same-document.md");
    let document_b = document(&markdown_b, "same-document.md");
    let context = ConflictContext::default();
    let no_groups = BTreeSet::new();

    let findings = detect_conflicts(&[
        source(0, &document_a, &markdown_a, &no_groups, &context),
        source(1, &document_b, &markdown_b, &no_groups, &context),
    ])
    .unwrap();
    assert_eq!(findings.len(), 1);
}

#[test]
fn different_occurrences_of_the_same_heading_path_do_not_join() {
    let markdown_a =
        table("Agent", "Port", "7005").replacen("# Guide", "# Guide\n\n## Repeated", 1);
    let markdown_b = concat!(
        "# Guide\n\n## Repeated\n\nEarlier section.\n\n",
        "## Repeated\n\n| Entity | Attribute | Value |\n",
        "| --- | --- | --- |\n| Agent | Port | 7006 |\n"
    );
    let document_a = document(&markdown_a, "doc-a.md");
    let document_b = document(markdown_b, "doc-b.md");
    let context = ConflictContext::default();
    let group = BTreeSet::from(["near-group".to_owned()]);

    assert!(
        detect_conflicts(&[
            source(0, &document_a, &markdown_a, &group, &context),
            source(1, &document_b, markdown_b, &group, &context),
        ])
        .unwrap()
        .is_empty()
    );
}

#[test]
fn unknown_or_different_context_and_unlisted_groups_do_not_join() {
    let markdown_a = table("Agent", "Port", "7005");
    let markdown_b = table("Agent", "Port", "7006");
    let document_a = document(&markdown_a, "doc-a.md");
    let document_b = document(&markdown_b, "doc-b.md");
    let group_a = BTreeSet::from(["group-a".to_owned()]);
    let group_b = BTreeSet::from(["group-b".to_owned()]);
    let context_a = ConflictContext::default();
    let context_b = ConflictContext {
        product: Some("product".to_owned()),
        ..ConflictContext::default()
    };

    assert!(
        detect_conflicts(&[
            source(0, &document_a, &markdown_a, &group_a, &context_a),
            source(1, &document_b, &markdown_b, &group_b, &context_a),
        ])
        .unwrap()
        .is_empty()
    );
    assert!(
        detect_conflicts(&[
            source(0, &document_a, &markdown_a, &group_a, &context_a),
            source(1, &document_b, &markdown_b, &group_a, &context_b),
        ])
        .unwrap()
        .is_empty()
    );
}

#[test]
fn equal_values_and_different_attributes_are_not_conflicts() {
    let markdown_a = table("Agent", "Port", "7005");
    let markdown_equal = table("Agent", "Port", "7005");
    let markdown_other_attribute = table("Agent", "Version", "7006");
    let document_a = document(&markdown_a, "doc-a.md");
    let document_equal = document(&markdown_equal, "doc-b.md");
    let document_other_attribute = document(&markdown_other_attribute, "doc-c.md");
    let context = ConflictContext::default();
    let group = BTreeSet::from(["near-group".to_owned()]);

    assert!(
        detect_conflicts(&[
            source(0, &document_a, &markdown_a, &group, &context),
            source(1, &document_equal, &markdown_equal, &group, &context),
        ])
        .unwrap()
        .is_empty()
    );
    assert!(
        detect_conflicts(&[
            source(0, &document_a, &markdown_a, &group, &context),
            source(
                1,
                &document_other_attribute,
                &markdown_other_attribute,
                &group,
                &context,
            ),
        ])
        .unwrap()
        .is_empty()
    );
}

#[test]
fn malformed_rows_and_unsupported_three_column_headers_are_ignored() {
    let valid = table("Agent", "Port", "7005");
    let malformed_row = table("Agent", "Port", "7006")
        .replace("| Agent | Port | 7006 |", "| Agent | Port | 7006 | extra |");
    let unsupported_header = table("Agent", "Port", "7006").replace("Attribute", "Property");
    let parameter_valid = parameter_table("7005");
    let unsupported_parameter_header = parameter_table("7006").replace("Parameter", "Setting");
    let context = ConflictContext::default();
    let group = BTreeSet::from(["near-group".to_owned()]);

    for (index, (baseline, malformed)) in [
        (&valid, malformed_row),
        (&valid, unsupported_header),
        (&parameter_valid, unsupported_parameter_header),
    ]
    .into_iter()
    .enumerate()
    {
        let baseline_document = document(baseline, &format!("valid-{index}.md"));
        let malformed_document = document(&malformed, &format!("malformed-{index}.md"));
        assert!(
            detect_conflicts(&[
                source(0, &baseline_document, baseline, &group, &context),
                source(1, &malformed_document, &malformed, &group, &context),
            ])
            .unwrap()
            .is_empty()
        );
    }
}

#[test]
fn escaped_pipes_remain_inside_their_table_cell() {
    let markdown_a = table("Agent", "Command", "read \\| write");
    let markdown_b = table("Agent", "Command", "read \\| send");
    let document_a = document(&markdown_a, "doc-a.md");
    let document_b = document(&markdown_b, "doc-b.md");
    let context = ConflictContext::default();
    let group = BTreeSet::from(["near-group".to_owned()]);
    let findings = detect_conflicts(&[
        source(0, &document_a, &markdown_a, &group, &context),
        source(1, &document_b, &markdown_b, &group, &context),
    ])
    .unwrap();

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].attribute, "Command");
    assert_eq!(
        findings[0].values,
        BTreeSet::from(["read | send".to_owned(), "read | write".to_owned()])
    );
}

#[test]
fn one_revision_rejects_inconsistent_context_and_group_membership() {
    let markdown = table("Agent", "Port", "7005");
    let document = document(&markdown, "same-revision.md");
    let group_a = BTreeSet::from(["group-a".to_owned()]);
    let group_b = BTreeSet::from(["group-b".to_owned()]);
    let context_a = ConflictContext::default();
    let context_b = ConflictContext {
        product: Some("product".to_owned()),
        ..ConflictContext::default()
    };

    assert_eq!(
        detect_conflicts(&[
            source(0, &document, &markdown, &group_a, &context_a),
            source(1, &document, &markdown, &group_a, &context_b),
        ])
        .unwrap_err(),
        "one revision has inconsistent canonical source data"
    );
    assert_eq!(
        detect_conflicts(&[
            source(0, &document, &markdown, &group_a, &context_a),
            source(1, &document, &markdown, &group_b, &context_a),
        ])
        .unwrap_err(),
        "one revision has inconsistent canonical source data"
    );

    let mut other_document = document.clone();
    other_document.source_metadata.title = Some("different metadata".to_owned());
    assert_eq!(
        detect_conflicts(&[
            source(0, &document, &markdown, &group_a, &context_a),
            source(1, &other_document, &markdown, &group_a, &context_a),
        ])
        .unwrap_err(),
        "one revision has inconsistent canonical source data"
    );

    let other_markdown = table("Agent", "Port", "7006");
    assert_eq!(
        detect_conflicts(&[
            source(0, &document, &markdown, &group_a, &context_a),
            source(1, &document, &other_markdown, &group_a, &context_a),
        ])
        .unwrap_err(),
        "one revision has inconsistent canonical source data"
    );
}

#[test]
fn same_revision_requires_the_request_cached_canonical_references() {
    let markdown = table("Agent", "Port", "7005");
    let copied_markdown = markdown.clone();
    let canonical = document(&markdown, "same-revision.md");
    let copied_canonical = canonical.clone();
    let groups = BTreeSet::new();
    let context = ConflictContext::default();

    assert_eq!(
        detect_conflicts(&[
            source(0, &canonical, &markdown, &groups, &context),
            source(1, &copied_canonical, &copied_markdown, &groups, &context),
        ])
        .unwrap_err(),
        "one revision has inconsistent canonical source data"
    );
}

#[test]
fn invalid_conflict_sources_are_refused_one_condition_at_a_time() {
    let markdown = table("Agent", "Port", "7005");
    let source_document = document(&markdown, "doc-a.md");
    let group = BTreeSet::from(["near-group".to_owned()]);
    let context = ConflictContext::default();

    let mut invalid = source(0, &source_document, &markdown, &group, &context);
    invalid.document_id = "";
    assert_invalid_source(invalid);

    let mut blank_revision = source_document.clone();
    blank_revision.revision_id.clear();
    assert_invalid_source(source(0, &blank_revision, &markdown, &group, &context));

    let mut invalid = source(0, &source_document, &markdown, &group, &context);
    invalid.document_id = "different-document";
    assert_invalid_source(invalid);

    let mut invalid = source(0, &source_document, &markdown, &group, &context);
    invalid.revision_id = "different-revision";
    assert_invalid_source(invalid);

    let blank_group = BTreeSet::from(["  ".to_owned()]);
    assert_invalid_source(source(
        0,
        &source_document,
        &markdown,
        &blank_group,
        &context,
    ));

    let mut invalid = source(0, &source_document, &markdown, &group, &context);
    invalid.proposed_extent = Span { start: 0, end: 0 };
    assert_invalid_source(invalid);

    let mut invalid = source(0, &source_document, &markdown, &group, &context);
    invalid.proposed_extent = Span {
        start: 0,
        end: usize::MAX,
    };
    assert_invalid_source(invalid);

    let unicode = "é";
    let unicode_document = document(unicode, "unicode.md");
    let mut invalid = source(0, &unicode_document, unicode, &group, &context);
    invalid.proposed_extent = Span { start: 0, end: 1 };
    assert_invalid_source(invalid);
}

#[test]
fn three_distinct_values_keep_the_conflict_and_deduplicate_table_spans() {
    let markdown_a = multi_table(&["7005", "7006"]);
    let markdown_b = table("Agent", "Port", "7007");
    let document_a = document(&markdown_a, "doc-a.md");
    let document_b = document(&markdown_b, "doc-b.md");
    let context = ConflictContext::default();
    let group = BTreeSet::from(["near-group".to_owned()]);
    let findings = detect_conflicts(&[
        source(0, &document_a, &markdown_a, &group, &context),
        source(1, &document_b, &markdown_b, &group, &context),
    ])
    .unwrap();

    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].values,
        BTreeSet::from(["7005".to_owned(), "7006".to_owned(), "7007".to_owned()])
    );
    assert_eq!(findings[0].table_spans[&0].len(), 1);
}

#[test]
fn repeated_candidate_observations_are_deduplicated() {
    let markdown = table("Agent", "Port", "7005");
    let document = document(&markdown, "doc-a.md");
    let context = ConflictContext::default();
    let group = BTreeSet::from(["near-group".to_owned()]);

    assert!(
        detect_conflicts(&[
            source(0, &document, &markdown, &group, &context),
            source(0, &document, &markdown, &group, &context),
        ])
        .unwrap()
        .is_empty()
    );
}

#[test]
fn unsupported_headers_and_marker_bearing_rows_are_ignored() {
    let unsupported = concat!(
        "# Guide\n\n| Entity | Attribute | Value | Notes |\n",
        "| --- | --- | --- | --- |\n| Agent | Port | 7005 | current |\n"
    );
    let conditional = table("Agent", "Port", "7005")
        .replace("| Agent | Port | 7005 |", "| Agent | Port | must be 7005 |");
    let other = table("Agent", "Port", "7006");
    let unsupported_document = document(unsupported, "unsupported.md");
    let conditional_document = document(&conditional, "conditional.md");
    let other_document = document(&other, "other.md");
    let context = ConflictContext::default();
    let group = BTreeSet::from(["near-group".to_owned()]);

    assert!(
        detect_conflicts(&[
            source(0, &unsupported_document, unsupported, &group, &context),
            source(1, &other_document, &other, &group, &context),
        ])
        .unwrap()
        .is_empty()
    );
    assert!(
        detect_conflicts(&[
            source(0, &conditional_document, &conditional, &group, &context),
            source(1, &other_document, &other, &group, &context),
        ])
        .unwrap()
        .is_empty()
    );
}
