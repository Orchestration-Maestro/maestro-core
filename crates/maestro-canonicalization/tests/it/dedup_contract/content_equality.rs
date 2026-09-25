//! What groups occurrences: equal content in a stable order, canonical
//! equality apart from the original syntax, and never content, metadata or
//! structure whose meaning differs.
use super::batch_inputs::{document, scope};
use maestro_canonicalization::{
    CanonicalizeInput, DedupInput, ExtractorBlock, Representation, SourceSpan, WarningPolicy,
    canonicalize, group_exact,
};
use serde_json::json;

#[test]
fn equal_content_retains_independent_unchanged_occurrences_in_stable_order() {
    let text = "# Title\n\nDo not delete 9.0.22.\n";
    let first = document(text, "source-a");
    let second = document(text, "source-b");
    assert_ne!(first.document_id, second.document_id);
    let before = serde_json::to_vec(&(&first, &second)).unwrap();
    let authorization = scope(&[&first, &second]);
    let first_input = DedupInput {
        document: &first,
        markdown: text,
    };
    let second_input = DedupInput {
        document: &second,
        markdown: text,
    };
    let result = group_exact(
        &authorization,
        &[first_input, second_input],
        WarningPolicy::Preserve,
    )
    .unwrap();
    let reversed = group_exact(
        &authorization,
        &[second_input, first_input],
        WarningPolicy::Preserve,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&result).unwrap(),
        serde_json::to_vec(&reversed).unwrap()
    );
    assert_eq!(result.occurrences.len(), 2);
    assert_eq!(result.groups.len(), 2);
    for group in &result.groups {
        assert_eq!(group.occurrence_indices, [0, 1]);
    }
    assert_ne!(result.groups[0].group_id, result.groups[1].group_id);
    for occurrence in &result.occurrences {
        assert_eq!(occurrence.markdown, text);
        assert_eq!(occurrence.original_hash, first.content_hash);
        assert!(occurrence.document.access_policy.is_none());
        assert!(!occurrence.document.warnings.is_empty());
    }
    assert_eq!(before, serde_json::to_vec(&(&first, &second)).unwrap());
}

#[test]
fn canonical_equality_is_separate_from_original_syntax_and_source_offsets() {
    let atx = "# Title\n\nKeep **bold** and [link](https://example.invalid/path).\n";
    let setext = "Title\n=====\n\nKeep **bold** and [link](https://example.invalid/path).\n";
    let from_atx = document(atx, "a");
    let from_setext = document(setext, "b");
    let authorization = scope(&[&from_atx, &from_setext]);
    let result = group_exact(
        &authorization,
        &[
            DedupInput {
                document: &from_atx,
                markdown: atx,
            },
            DedupInput {
                document: &from_setext,
                markdown: setext,
            },
        ],
        WarningPolicy::Preserve,
    )
    .unwrap();
    assert_ne!(
        result.occurrences[0].original_hash,
        result.occurrences[1].original_hash
    );
    assert_eq!(
        result.occurrences[0].canonical_hash,
        result.occurrences[1].canonical_hash
    );
    assert_eq!(result.groups.len(), 3);
    let canonical = result
        .groups
        .iter()
        .find(|group| group.representation == Representation::Canonical)
        .unwrap();
    assert_eq!(canonical.occurrence_indices, [0, 1]);
}

#[test]
fn structured_comparison_preserves_meaning_sensitive_content_and_hierarchy() {
    let mutations = [
        ("negation", "Do not delete.\n", "Do delete.\n"),
        ("version", "Version 9.0.22\n", "Version 9.0.21\n"),
        ("identifier case", "repoName\n", "reponame\n"),
        ("punctuation", "Wait; retry.\n", "Wait retry.\n"),
        ("unicode form", "café\n", "cafe\u{301}\n"),
        ("heading level", "# Header\n", "## Header\n"),
        ("heading anchor", "# Header {#one}\n", "# Header {#two}\n"),
        ("heading class", "# Header {.one}\n", "# Header {.two}\n"),
        ("inline code", "word\n", "`word`\n"),
        ("emphasis", "word\n", "**word**\n"),
        ("quotation", "word\n", "> word\n"),
        ("code indentation", "```py\n  a\n```\n", "```py\n a\n```\n"),
        ("code language", "```py\na\n```\n", "```sh\na\n```\n"),
        ("list start", "1. item\n", "2. item\n"),
        ("list nesting", "- a\n  - b\n", "- a\n- b\n"),
        (
            "table header",
            "| A |\n|---|\n| x |\n",
            "| B |\n|---|\n| x |\n",
        ),
        (
            "table alignment",
            "| A |\n|:---|\n| x |\n",
            "| A |\n|---:|\n| x |\n",
        ),
        (
            "table cells",
            "| A | B |\n|---|---|\n| x | y |\n",
            "| A | B |\n|---|---|\n| y | x |\n",
        ),
        (
            "link destination",
            "[a](https://example.invalid/a)\n",
            "[a](https://example.invalid/b)\n",
        ),
        ("task checkbox", "- [ ] done\n", "- [x] done\n"),
        ("raw html", "<div>a</div>\n", "<div>b</div>\n"),
    ];
    for (name, left, right) in mutations {
        let left_doc = document(left, "left");
        let right_doc = document(right, "right");
        let authorization = scope(&[&left_doc, &right_doc]);
        let result = group_exact(
            &authorization,
            &[
                DedupInput {
                    document: &left_doc,
                    markdown: left,
                },
                DedupInput {
                    document: &right_doc,
                    markdown: right,
                },
            ],
            WarningPolicy::Preserve,
        )
        .unwrap();
        assert_eq!(result.groups.len(), 4, "{name}");
        assert!(
            result
                .groups
                .iter()
                .all(|group| group.occurrence_indices.len() == 1),
            "{name}"
        );
        assert_ne!(
            result.occurrences[0].canonical_hash, result.occurrences[1].canonical_hash,
            "{name}"
        );
    }
}

/// One change to the source metadata or the extractor structure of an input.
type SourceChange = fn(&mut CanonicalizeInput<'_>);

#[test]
fn source_metadata_and_extractor_structure_are_not_silently_discarded() {
    let text = "# Title\n\nContent.\n";
    let changes: [(&str, SourceChange); 4] = [
        ("title", |right| {
            right.metadata.title = Some("Another title".into());
        }),
        ("language", |right| {
            right.metadata.language = Some("fr".into());
        }),
        ("extra", |right| {
            right
                .metadata
                .extra
                .insert("version".into(), json!("9.0.21"));
        }),
        ("extractor", |right| {
            right.extractor_blocks[0].structured_content =
                json!({"kind": "code", "value": "Content."});
        }),
    ];
    for (name, change) in changes {
        let mut left = CanonicalizeInput::new(text, "left");
        left.metadata.title = Some("Title".into());
        left.metadata.language = Some("en".into());
        left.metadata
            .extra
            .insert("version".into(), json!("9.0.22"));
        left.extractor_blocks.push(ExtractorBlock {
            extractor_id: "block".into(),
            markdown_spans: vec![SourceSpan {
                start: 0,
                end: text.len(),
            }],
            original_locations: Vec::new(),
            structured_content: json!({"kind": "paragraph", "value": "Content."}),
        });
        let mut right = left.clone();
        right.identity_key = "right";
        change(&mut right);
        let left_doc = canonicalize(left).unwrap();
        let right_doc = canonicalize(right).unwrap();
        let authorization = scope(&[&left_doc, &right_doc]);
        let result = group_exact(
            &authorization,
            &[
                DedupInput {
                    document: &left_doc,
                    markdown: text,
                },
                DedupInput {
                    document: &right_doc,
                    markdown: text,
                },
            ],
            WarningPolicy::Preserve,
        )
        .unwrap();
        assert_eq!(
            result.occurrences[0].original_hash,
            result.occurrences[1].original_hash
        );
        assert_ne!(
            result.occurrences[0].canonical_hash, result.occurrences[1].canonical_hash,
            "{name}"
        );
        assert_eq!(result.groups.len(), 3, "{name}");
    }
}
