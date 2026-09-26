//! Tests of chunk assembly, prepared-input groups and replay validation.
use super::identity::{PreparedGroups, insert_prepared_group};
use super::validation::{invalid_chunks, validate_chunks, validate_coverage};
use super::*;
use crate::dedup::{DedupInput, DedupScope, RevisionKey, WarningPolicy};
use crate::document::CanonicalDocument;
use crate::error::Error;
use crate::model::{CanonicalizeInput, Severity};
use crate::pipeline::canonicalize;
use crate::replay::validate_document;
use crate::tokenizer::TokenCounter;
use std::cell::Cell;

mod counter;
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

/// The stand-in counter of these tests: `fake_count` IDs, one per character between BOS 0 and
/// EOS 2, under a contract ID of the test's choosing. It records its calls, and its `verify`
/// can fail at a chosen call.
struct TestCounter {
    /// The contract every identity of its batches carries.
    contract_id: &'static str,
    /// The `verify` call, counted from one, that fails; `None` never fails.
    failing_verify: Option<usize>,
    /// How many times `verify` ran.
    verifications: Cell<usize>,
    /// How many inputs it tokenized.
    tokenizations: Cell<usize>,
}

impl TestCounter {
    /// A counter under `contract_id` whose `verify` always passes.
    fn new(contract_id: &'static str) -> Self {
        Self {
            contract_id,
            failing_verify: None,
            verifications: Cell::new(0),
            tokenizations: Cell::new(0),
        }
    }
}

impl TokenCounter for TestCounter {
    fn contract_id(&self) -> &str {
        self.contract_id
    }

    fn verify(&self) -> Result<(), Error> {
        let call = self.verifications.get() + 1;
        self.verifications.set(call);
        if self.failing_verify == Some(call) {
            return Err(Error("test counter artifacts changed".into()));
        }
        Ok(())
    }

    fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error> {
        self.tokenizations.set(self.tokenizations.get() + 1);
        let characters = input.chars().map(u32::from);
        Ok([0].into_iter().chain(characters).chain([2]).collect())
    }
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
    let never_called = TestCounter::new("test/unqualified");
    let denied = chunk_documents(
        &scope(&[&valid]),
        &inputs,
        WarningPolicy::Preserve,
        &never_called,
    )
    .unwrap_err();
    assert!(!denied.to_string().contains("private"));
    assert!(
        chunk_documents(
            &scope(&[]),
            &inputs[..1],
            WarningPolicy::Preserve,
            &never_called
        )
        .is_err()
    );
    assert!(
        chunk_documents(
            &scope(&[&valid, &invalid]),
            &inputs,
            WarningPolicy::Preserve,
            &never_called
        )
        .is_err()
    );
    assert!(
        chunk_documents(
            &scope(&[&valid]),
            &[inputs[0], inputs[0]],
            WarningPolicy::Preserve,
            &never_called
        )
        .is_err()
    );
    assert_eq!(never_called.verifications.get(), 0);
    assert_eq!(never_called.tokenizations.get(), 0);
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
    let never_called = TestCounter::new("test/unqualified");
    assert!(chunk_documents(&scope, &[input], WarningPolicy::Reject, &never_called).is_err());
    let preserved = chunk_documents(
        &scope,
        &[input],
        WarningPolicy::Preserve,
        &TestCounter::new("test/unqualified"),
    )
    .unwrap();
    assert!(
        !preserved.deduplication.occurrences[0]
            .document
            .warnings
            .is_empty()
    );
    let empty = chunk_documents(&scope, &[], WarningPolicy::Reject, &never_called).unwrap();
    assert!(empty.chunks.is_empty());
    for markdown in ["", "---\ntitle: Metadata\n---\n"] {
        let doc = canonicalize(CanonicalizeInput::new(markdown, "rejected-empty")).unwrap();
        let grant = self::scope(&[&doc]);
        assert!(
            chunk_documents(
                &grant,
                &[DedupInput {
                    document: &doc,
                    markdown
                }],
                WarningPolicy::Preserve,
                &never_called
            )
            .is_err()
        );
    }
    let markdown = "-\n";
    let doc = canonicalize(CanonicalizeInput::new(markdown, "empty-item")).unwrap();
    assert!(
        validate_document(&doc, markdown)
            .iter()
            .all(|finding| finding.severity != Severity::Error)
    );
    let grant = self::scope(&[&doc]);
    let result = chunk_documents(
        &grant,
        &[DedupInput {
            document: &doc,
            markdown,
        }],
        WarningPolicy::Preserve,
        &never_called,
    )
    .unwrap();
    assert!(result.chunks.is_empty());
    assert!(result.documents[0].no_searchable_content);
    assert_eq!(never_called.tokenizations.get(), 0);
}

#[test]
fn counts_are_cached_by_complete_bytes_only_within_one_authorized_call() {
    let markdown = "equal\n";
    let cache_a = canonicalize(CanonicalizeInput::new(markdown, "cache-a")).unwrap();
    let cache_b = canonicalize(CanonicalizeInput::new(markdown, "cache-b")).unwrap();
    let scope = scope(&[&cache_a, &cache_b]);
    let inputs = [
        DedupInput {
            document: &cache_a,
            markdown,
        },
        DedupInput {
            document: &cache_b,
            markdown,
        },
    ];
    let counter = TestCounter::new("test/unqualified");
    chunk_documents(&scope, &inputs, WarningPolicy::Preserve, &counter).unwrap();
    assert_eq!(counter.tokenizations.get(), 1);
    chunk_documents(&scope, &inputs, WarningPolicy::Preserve, &counter).unwrap();
    assert_eq!(counter.tokenizations.get(), 2);
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
        let batch = chunk_documents(
            &scope,
            &[DedupInput {
                document: &doc,
                markdown,
            }],
            WarningPolicy::Preserve,
            &TestCounter::new("test/unqualified"),
        )
        .unwrap();
        assert!(!batch.chunks.is_empty());
        assert_eq!(
            batch.documents[0].mapped.accounting.len(),
            doc.source_accounting.len()
        );
        assert!(
            batch
                .chunks
                .iter()
                .all(|chunk| chunk.content.token_count <= 700)
        );
        assert_eq!(before, serde_json::to_vec(&doc).unwrap());
    }
}
