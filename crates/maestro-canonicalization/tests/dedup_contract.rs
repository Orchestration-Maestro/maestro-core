//! Exact duplicate grouping: stable occurrences, authorized scope and whole-batch refusals.
#![cfg(test)]
use maestro_canonicalization::{
    CanonicalDocument, CanonicalizeInput, DedupInput, DedupScope, ExtractorBlock, OriginalLocation,
    Representation, RevisionKey, SourceSpan, ValidationStatus, WarningPolicy, canonicalize,
    group_exact,
};
use serde_json::json;
use std::ptr;

fn document(text: &str, identity: &str) -> CanonicalDocument {
    canonicalize(CanonicalizeInput::new(text, identity)).unwrap()
}

fn scope(documents: &[&CanonicalDocument]) -> DedupScope {
    DedupScope {
        tenant_id: "tenant-a".into(),
        workspace_id: "workspace-a".into(),
        authorized_revisions: documents
            .iter()
            .map(|doc| RevisionKey {
                document_id: doc.document_id.clone(),
                revision_id: doc.revision_id.clone(),
            })
            .collect(),
    }
}

#[test]
fn equal_content_retains_independent_unchanged_occurrences_in_stable_order() {
    let text = "# Title\n\nDo not delete 9.0.22.\n";
    let first = document(text, "source-a");
    let second = document(text, "source-b");
    assert_ne!(first.document_id, second.document_id);
    let before = serde_json::to_vec(&(&first, &second)).unwrap();
    let authorization = scope(&[&first, &second]);
    let a = DedupInput {
        document: &first,
        markdown: text,
    };
    let b = DedupInput {
        document: &second,
        markdown: text,
    };
    let result = group_exact(&authorization, &[a, b], WarningPolicy::Preserve).unwrap();
    let reversed = group_exact(&authorization, &[b, a], WarningPolicy::Preserve).unwrap();
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
    let a = document(atx, "a");
    let b = document(setext, "b");
    let authorization = scope(&[&a, &b]);
    let result = group_exact(
        &authorization,
        &[
            DedupInput {
                document: &a,
                markdown: atx,
            },
            DedupInput {
                document: &b,
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
        .find(|g| g.representation == Representation::Canonical)
        .unwrap();
    assert_eq!(canonical.occurrence_indices, [0, 1]);
}

#[test]
fn scope_and_current_authorization_bound_membership_without_deleting_history() {
    let text = "# Title\n\nSame source content.\n";
    let a = document(text, "a");
    let b = document(text, "b");
    let authorization = scope(&[&a, &b]);
    let inputs = [
        DedupInput {
            document: &a,
            markdown: text,
        },
        DedupInput {
            document: &b,
            markdown: text,
        },
    ];
    let original = group_exact(&authorization, &inputs, WarningPolicy::Preserve).unwrap();
    for (tenant, workspace) in [("tenant-b", "workspace-a"), ("tenant-a", "workspace-b")] {
        let mut changed = scope(&[&a, &b]);
        changed.tenant_id = tenant.into();
        changed.workspace_id = workspace.into();
        let result = group_exact(&changed, &inputs, WarningPolicy::Preserve).unwrap();
        assert!(
            original
                .groups
                .iter()
                .zip(&result.groups)
                .all(|(a, b)| a.group_id != b.group_id)
        );
    }
    let revoked = scope(&[&b]);
    assert!(group_exact(&revoked, &inputs, WarningPolicy::Preserve).is_err());
    let surviving = group_exact(&revoked, &inputs[1..], WarningPolicy::Preserve).unwrap();
    assert_eq!(surviving.occurrences.len(), 1);
    assert_eq!(surviving.occurrences[0].document.document_id, b.document_id);
    assert!(surviving.groups.iter().all(|g| g.occurrence_indices == [0]));
    assert_eq!(original.occurrences.len(), 2);
    assert_eq!(a.content_hash, b.content_hash);
}

#[test]
fn invalid_scope_duplicate_keys_warnings_and_tampering_refuse_the_batch() {
    let text = "# Title\n\nDo not delete.\n";
    let doc = document(text, "a");
    let authorization = scope(&[&doc]);
    let input = DedupInput {
        document: &doc,
        markdown: text,
    };
    for (tenant, workspace) in [("", "a"), ("a", " \t")] {
        let mut invalid = scope(&[&doc]);
        invalid.tenant_id = tenant.into();
        invalid.workspace_id = workspace.into();
        assert!(group_exact(&invalid, &[input], WarningPolicy::Preserve).is_err());
    }
    assert!(group_exact(&authorization, &[input, input], WarningPolicy::Preserve).is_err());
    assert!(group_exact(&authorization, &[input], WarningPolicy::Reject).is_err());
    assert!(
        group_exact(
            &authorization,
            &[DedupInput {
                document: &doc,
                markdown: "changed"
            }],
            WarningPolicy::Preserve
        )
        .is_err()
    );
    let mut forged = doc.clone();
    forged.blocks[0].retrieval_text = "forged".into();
    assert!(
        group_exact(
            &authorization,
            &[DedupInput {
                document: &forged,
                markdown: text
            }],
            WarningPolicy::Preserve
        )
        .is_err()
    );
    let empty = document("", "empty");
    let empty_scope = scope(&[&empty]);
    assert!(
        group_exact(
            &empty_scope,
            &[DedupInput {
                document: &empty,
                markdown: ""
            }],
            WarningPolicy::Preserve
        )
        .is_err()
    );
    let no_inputs = group_exact(&authorization, &[], WarningPolicy::Reject).unwrap();
    assert!(no_inputs.occurrences.is_empty() && no_inputs.groups.is_empty());
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
        let a = document(left, "left");
        let b = document(right, "right");
        let authorization = scope(&[&a, &b]);
        let result = group_exact(
            &authorization,
            &[
                DedupInput {
                    document: &a,
                    markdown: left,
                },
                DedupInput {
                    document: &b,
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
                .all(|g| g.occurrence_indices.len() == 1),
            "{name}"
        );
        assert_ne!(
            result.occurrences[0].canonical_hash, result.occurrences[1].canonical_hash,
            "{name}"
        );
    }
}

#[test]
fn source_metadata_and_extractor_structure_are_not_silently_discarded() {
    let text = "# Title\n\nContent.\n";
    for name in ["title", "language", "extra", "extractor"] {
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
        match name {
            "title" => right.metadata.title = Some("Another title".into()),
            "language" => right.metadata.language = Some("fr".into()),
            "extra" => {
                right
                    .metadata
                    .extra
                    .insert("version".into(), json!("9.0.21"));
            }
            "extractor" => {
                right.extractor_blocks[0].structured_content =
                    json!({"kind": "code", "value": "Content."});
            }
            _ => unreachable!(),
        }
        let a = canonicalize(left).unwrap();
        let b = canonicalize(right).unwrap();
        let authorization = scope(&[&a, &b]);
        let result = group_exact(
            &authorization,
            &[
                DedupInput {
                    document: &a,
                    markdown: text,
                },
                DedupInput {
                    document: &b,
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

#[test]
fn policies_coordinates_and_revision_history_remain_per_occurrence() {
    let text = "# Title\n\nSame content.\n";
    let mut old = sourced_input(text);
    check_a_clean_revision_groups_under_reject(text, &old);
    old.extractor_blocks.push(extractor_block(text));
    let updated = updated_revision(&old);
    let a = canonicalize(old).unwrap();
    let b = canonicalize(updated).unwrap();
    check_two_revisions_of_one_document(&a, &b);
    let before = serde_json::to_vec(&(&a, &b)).unwrap();
    check_each_occurrence_keeps_its_own_revision(text, &a, &b);
    assert_eq!(before, serde_json::to_vec(&(&a, &b)).unwrap());
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
fn check_two_revisions_of_one_document(a: &CanonicalDocument, b: &CanonicalDocument) {
    for doc in [a, b] {
        assert_eq!(doc.validation_status, ValidationStatus::ValidWithWarnings);
        assert_eq!(
            doc.warnings
                .iter()
                .map(|w| w.code.as_str())
                .collect::<Vec<_>>(),
            ["extractor_payload_retained"]
        );
    }
    assert_eq!(a.document_id, b.document_id);
    assert_ne!(a.revision_id, b.revision_id);
}

/// Grouped together, each occurrence keeps its own revision's policy,
/// extractor content and run metadata.
fn check_each_occurrence_keeps_its_own_revision(
    text: &str,
    a: &CanonicalDocument,
    b: &CanonicalDocument,
) {
    let authorization = scope(&[a, b]);
    let result = group_exact(
        &authorization,
        &[
            DedupInput {
                document: a,
                markdown: text,
            },
            DedupInput {
                document: b,
                markdown: text,
            },
        ],
        WarningPolicy::Preserve,
    )
    .unwrap();
    assert_eq!(result.groups.len(), 2);
    assert!(result.groups.iter().all(|g| g.occurrence_indices == [0, 1]));
    for expected in [a, b] {
        let occurrence = result
            .occurrences
            .iter()
            .find(|o| o.document.revision_id == expected.revision_id)
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

#[test]
fn unauthorized_inputs_do_not_leak_replay_or_source_details() {
    let text = "# Private marker\n\nContent.\n";
    let valid = document(text, "private-source-marker");
    let mut forged = valid.clone();
    forged.blocks.clear();
    let authorization = scope(&[]);
    let refused = |doc| {
        group_exact(
            &authorization,
            &[DedupInput {
                document: doc,
                markdown: text,
            }],
            WarningPolicy::Preserve,
        )
        .unwrap_err()
        .to_string()
    };
    assert_eq!(refused(&valid), refused(&forged));
    assert!(!refused(&valid).contains("Private marker"));
    assert!(!refused(&valid).contains(&valid.document_id));
}
