//! Tests of chunk assembly, prepared-input groups and replay validation.
use super::identity::{PreparedGroups, insert_prepared_group};
use super::validation::{validate_chunks, validate_coverage};
use super::*;
use crate::{
    CanonicalDocument, CanonicalizeInput, DedupInput, DedupScope, Error, RevisionKey,
    WarningPolicy, canonicalize,
};

mod identity;
mod replay;

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

fn fake_count(input: &str) -> usize {
    input.chars().count() + 2
}

#[test]
fn authorization_and_all_replays_precede_every_counter_call() {
    let markdown = "private body\n";
    let valid = canonicalize(CanonicalizeInput::new(markdown, "valid")).unwrap();
    let mut invalid = canonicalize(CanonicalizeInput::new(markdown, "invalid")).unwrap();
    invalid.blocks[0].retrieval_text = "tampered".into();
    let inputs = [
        DedupInput {
            document: &valid,
            markdown,
        },
        DedupInput {
            document: &invalid,
            markdown,
        },
    ];
    let mut never_called =
        |_: &str| -> Result<usize, Error> { panic!("ineligible source reached counter") };
    let denied = chunk_with_count(
        &scope(&[&valid]),
        &inputs,
        WarningPolicy::Preserve,
        "test/unqualified",
        &mut never_called,
    )
    .unwrap_err();
    assert!(!denied.to_string().contains("private"));
    assert!(
        chunk_with_count(
            &scope(&[]),
            &inputs[..1],
            WarningPolicy::Preserve,
            "test/unqualified",
            &mut never_called
        )
        .is_err()
    );
    assert!(
        chunk_with_count(
            &scope(&[&valid, &invalid]),
            &inputs,
            WarningPolicy::Preserve,
            "test/unqualified",
            &mut never_called
        )
        .is_err()
    );
    assert!(
        chunk_with_count(
            &scope(&[&valid]),
            &[inputs[0], inputs[0]],
            WarningPolicy::Preserve,
            "test/unqualified",
            &mut never_called
        )
        .is_err()
    );
}

#[test]
fn warning_policy_and_empty_content_are_explicit() {
    let markdown = "<div>raw</div>\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "html")).unwrap();
    let scope = scope(&[&doc]);
    let input = DedupInput {
        document: &doc,
        markdown,
    };
    let mut never_called =
        |_: &str| -> Result<usize, Error> { panic!("rejected warning reached counter") };
    assert!(
        chunk_with_count(
            &scope,
            &[input],
            WarningPolicy::Reject,
            "test/unqualified",
            &mut never_called
        )
        .is_err()
    );
    let preserved = chunk_with_count(
        &scope,
        &[input],
        WarningPolicy::Preserve,
        "test/unqualified",
        &mut |input| Ok(fake_count(input)),
    )
    .unwrap();
    assert!(
        !preserved.deduplication.occurrences[0]
            .document
            .warnings
            .is_empty()
    );
    let empty = chunk_with_count(
        &scope,
        &[],
        WarningPolicy::Reject,
        "test/unqualified",
        &mut never_called,
    )
    .unwrap();
    assert!(empty.chunks.is_empty());
    for markdown in ["", "---\ntitle: Metadata\n---\n"] {
        let doc = canonicalize(CanonicalizeInput::new(markdown, "rejected-empty")).unwrap();
        let grant = super::tests::scope(&[&doc]);
        assert!(
            chunk_with_count(
                &grant,
                &[DedupInput {
                    document: &doc,
                    markdown
                }],
                WarningPolicy::Preserve,
                "test/unqualified",
                &mut never_called
            )
            .is_err()
        );
    }
    let markdown = "-\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "empty-item")).unwrap();
    assert!(
        crate::validate_document(&doc, markdown)
            .iter()
            .all(|finding| finding.severity != crate::Severity::Error)
    );
    let grant = super::tests::scope(&[&doc]);
    let result = chunk_with_count(
        &grant,
        &[DedupInput {
            document: &doc,
            markdown,
        }],
        WarningPolicy::Preserve,
        "test/unqualified",
        &mut never_called,
    )
    .unwrap();
    assert!(result.chunks.is_empty());
    assert!(result.documents[0].no_searchable_content);
}

#[test]
fn counts_are_cached_by_complete_bytes_only_within_one_authorized_call() {
    let markdown = "equal\n";
    let a = canonicalize(CanonicalizeInput::new(markdown, "cache-a")).unwrap();
    let b = canonicalize(CanonicalizeInput::new(markdown, "cache-b")).unwrap();
    let scope = scope(&[&a, &b]);
    let inputs = [
        DedupInput {
            document: &a,
            markdown,
        },
        DedupInput {
            document: &b,
            markdown,
        },
    ];
    let mut calls = 0;
    let mut count = |text: &str| {
        calls += 1;
        Ok(fake_count(text))
    };
    chunk_with_count(
        &scope,
        &inputs,
        WarningPolicy::Preserve,
        "test/unqualified",
        &mut count,
    )
    .unwrap();
    assert_eq!(calls, 1);
    let mut count = |text: &str| {
        calls += 1;
        Ok(fake_count(text))
    };
    chunk_with_count(
        &scope,
        &inputs,
        WarningPolicy::Preserve,
        "test/unqualified",
        &mut count,
    )
    .unwrap();
    assert_eq!(calls, 2);
}

#[test]
fn structural_matrix_survives_full_authorization_and_dual_accounting() {
    for markdown in [
        "# T\r\n\r\nCafé e\u{301} 😀 &amp; \\* literal\0.\r\n",
        "- first\n  second\n  - nested\n\n    ```rust\n    x\n    y\n    ```\n",
        "- | A | B |\n  |---|---|\n  | one | two |\n",
        "Term\n: Definition\n\nText[^note].\n\n[^note]: Detail.\n",
        "> [!WARNING]\n> Be careful.\n\n<div>raw</div>\n",
        concat!(
            "~~deleted ^super^ ~sub~ text~~ and ![label][asset].\n\n",
            "[asset]: https://example.invalid/image.svg\n"
        ),
    ] {
        let doc = canonicalize(CanonicalizeInput::new(markdown, "matrix")).unwrap();
        let before = serde_json::to_vec(&doc).unwrap();
        let scope = scope(&[&doc]);
        let batch = chunk_with_count(
            &scope,
            &[DedupInput {
                document: &doc,
                markdown,
            }],
            WarningPolicy::Preserve,
            "test/unqualified",
            &mut |input| Ok(fake_count(input)),
        )
        .unwrap();
        assert!(!batch.chunks.is_empty());
        assert_eq!(
            batch.documents[0].mapped.accounting.len(),
            doc.source_accounting.len()
        );
        assert!(batch.chunks.iter().all(|c| c.content.token_count <= 700));
        assert_eq!(before, serde_json::to_vec(&doc).unwrap());
    }
}
