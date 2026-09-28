use super::super::conflicts::{detect::detect_conflicts, emit_conflicts};
use super::super::families::ConflictContext;
use super::conflicts::{document, source};
use maestro_kernel::artifact::Digest;
use maestro_kernel::evidence::{Passage, Span};
use std::collections::BTreeSet;

fn passage(document_id: &str, revision_id: &str, n: u32, span: Span, text: &str) -> Passage {
    Passage {
        n,
        section_id: Some("section".to_owned()),
        document_id: document_id.to_owned(),
        revision_id: revision_id.to_owned(),
        title: "Guide".to_owned(),
        section_path: vec!["Guide".to_owned()],
        version: None,
        source_ref: "corpus-path:guide.md".to_owned(),
        span,
        digest: Digest::of(text.as_bytes()),
        text: text.to_owned(),
        windowed: false,
        alternates: Vec::new(),
    }
}

#[test]
fn distinct_passages_emit_one_conflict_with_sorted_numbers() {
    let markdown_a =
        "# Guide\n\n| Entity | Attribute | Value |\n| --- | --- | --- |\n| Agent | Port | 7005 |\n";
    let markdown_b = markdown_a.replace("7005", "7006");
    let document_a = document(markdown_a, "conflict-a.md");
    let document_b = document(&markdown_b, "conflict-b.md");
    let groups = BTreeSet::from(["near-group".to_owned()]);
    let context = ConflictContext::default();
    let sources = [
        source(0, &document_a, markdown_a, &groups, &context),
        source(1, &document_b, &markdown_b, &groups, &context),
    ];
    let findings = detect_conflicts(&sources).unwrap();
    let passages = [
        passage(
            &document_a.document_id,
            &document_a.revision_id,
            2,
            Span {
                start: 0,
                end: markdown_a.len(),
            },
            markdown_a,
        ),
        passage(
            &document_b.document_id,
            &document_b.revision_id,
            1,
            Span {
                start: 0,
                end: markdown_b.len(),
            },
            &markdown_b,
        ),
    ];

    let emitted = emit_conflicts(&findings, &sources, &BTreeSet::from([0, 1]), &passages).unwrap();

    assert_eq!(emitted.conflicts.len(), 1);
    assert_eq!(emitted.conflicts[0].entity, "Agent");
    assert_eq!(emitted.conflicts[0].attribute, "Port");
    assert_eq!(emitted.conflicts[0].passages, [1, 2]);
    assert!(emitted.within_passage.is_empty());
}

#[test]
fn touching_conflict_tables_emit_a_within_passage_gap_not_a_singleton_conflict() {
    let markdown = concat!(
        "# Guide\n\n| Entity | Attribute | Value |\n",
        "| --- | --- | --- |\n| Agent | Port | 7005 |\n| Agent | Port | 7006 |\n"
    );
    let source_document = document(markdown, "one-revision.md");
    let groups = BTreeSet::new();
    let context = ConflictContext::default();
    let sources = [
        source(0, &source_document, markdown, &groups, &context),
        source(1, &source_document, markdown, &groups, &context),
    ];
    let findings = detect_conflicts(&sources).unwrap();
    let passages = [passage(
        &source_document.document_id,
        &source_document.revision_id,
        1,
        Span {
            start: 0,
            end: markdown.len(),
        },
        markdown,
    )];

    let emitted = emit_conflicts(&findings, &sources, &BTreeSet::from([0, 1]), &passages).unwrap();

    assert!(emitted.conflicts.is_empty());
    assert_eq!(emitted.within_passage.len(), 1);
    assert_eq!(emitted.within_passage[0].passage_number, 1);
    assert_eq!(emitted.within_passage[0].entity, "Agent");
    assert_eq!(emitted.within_passage[0].attribute, "Port");
}

#[test]
fn repeated_section_conflicts_deduplicate_per_passage_without_merging_attributes() {
    let markdown = concat!(
        "# Guide\n\n## Network\n\n",
        "| Entity | Attribute | Value |\n| --- | --- | --- |\n",
        "| Agent | Port | 7005 |\n| Agent | Port | 7006 |\n",
        "| Agent | Mode | fast |\n| Agent | Mode | safe |\n\n",
        "## Network\n\n",
        "| Entity | Attribute | Value |\n| --- | --- | --- |\n",
        "| Agent | Port | 7007 |\n| Agent | Port | 7008 |\n",
        "| Agent | Mode | active |\n| Agent | Mode | standby |\n"
    );
    let source_document = document(markdown, "repeated-sections.md");
    let no_groups = BTreeSet::new();
    let context = ConflictContext::default();
    let sources = [
        source(0, &source_document, markdown, &no_groups, &context),
        source(1, &source_document, markdown, &no_groups, &context),
    ];
    let findings = detect_conflicts(&sources).unwrap();
    assert_eq!(findings.len(), 4);
    let passages = [passage(
        &source_document.document_id,
        &source_document.revision_id,
        1,
        Span {
            start: 0,
            end: markdown.len(),
        },
        markdown,
    )];

    let emitted = emit_conflicts(&findings, &sources, &BTreeSet::from([0, 1]), &passages).unwrap();

    assert!(emitted.conflicts.is_empty());
    assert_eq!(
        emitted
            .within_passage
            .iter()
            .map(|finding| finding.attribute)
            .collect::<Vec<_>>(),
        ["Mode", "Port"]
    );
}

#[test]
fn a_partially_retained_conflict_is_refused() {
    let markdown_a =
        "# Guide\n\n| Entity | Attribute | Value |\n| --- | --- | --- |\n| Agent | Port | 7005 |\n";
    let markdown_b = markdown_a.replace("7005", "7006");
    let document_a = document(markdown_a, "conflict-a.md");
    let document_b = document(&markdown_b, "conflict-b.md");
    let groups = BTreeSet::from(["near-group".to_owned()]);
    let context = ConflictContext::default();
    let sources = [
        source(0, &document_a, markdown_a, &groups, &context),
        source(1, &document_b, &markdown_b, &groups, &context),
    ];
    let findings = detect_conflicts(&sources).unwrap();

    assert_eq!(
        emit_conflicts(&findings, &sources, &BTreeSet::from([0]), &[]).err(),
        Some("selection returned only one side of a conflict".to_owned())
    );
}

#[test]
fn rejects_incomplete_findings_and_un_numbered_table_passages() {
    let markdown_a =
        "# Guide\n\n| Entity | Attribute | Value |\n| --- | --- | --- |\n| Agent | Port | 7005 |\n";
    let markdown_b = markdown_a.replace("7005", "7006");
    let document_a = document(markdown_a, "conflict-a.md");
    let document_b = document(&markdown_b, "conflict-b.md");
    let groups = BTreeSet::from(["near-group".to_owned()]);
    let context = ConflictContext::default();
    let sources = [
        source(0, &document_a, markdown_a, &groups, &context),
        source(1, &document_b, &markdown_b, &groups, &context),
    ];
    let findings = detect_conflicts(&sources).unwrap();
    let mut incomplete = findings.clone();
    incomplete[0].values = BTreeSet::from(["7005".to_owned()]);
    assert_eq!(
        emit_conflicts(&incomplete, &sources, &BTreeSet::new(), &[]).err(),
        Some("conflict finding has fewer than two explicit values".to_owned())
    );

    let passages = [
        passage(
            &document_a.document_id,
            &document_a.revision_id,
            0,
            Span {
                start: 0,
                end: markdown_a.len(),
            },
            markdown_a,
        ),
        passage(
            &document_b.document_id,
            &document_b.revision_id,
            1,
            Span {
                start: 0,
                end: markdown_b.len(),
            },
            &markdown_b,
        ),
    ];
    assert_eq!(
        emit_conflicts(&findings, &sources, &BTreeSet::from([0, 1]), &passages).err(),
        Some("conflict table has ambiguous passage provenance".to_owned())
    );
}

#[test]
fn omitted_conflict_units_emit_no_conflict_signal() {
    let markdown_a =
        "# Guide\n\n| Entity | Attribute | Value |\n| --- | --- | --- |\n| Agent | Port | 7005 |\n";
    let markdown_b = markdown_a.replace("7005", "7006");
    let document_a = document(markdown_a, "conflict-a.md");
    let document_b = document(&markdown_b, "conflict-b.md");
    let groups = BTreeSet::from(["near-group".to_owned()]);
    let context = ConflictContext::default();
    let sources = [
        source(0, &document_a, markdown_a, &groups, &context),
        source(1, &document_b, &markdown_b, &groups, &context),
    ];
    let findings = detect_conflicts(&sources).unwrap();
    let emitted = emit_conflicts(&findings, &sources, &BTreeSet::new(), &[]).unwrap();

    assert!(emitted.conflicts.is_empty());
    assert!(emitted.within_passage.is_empty());
}
