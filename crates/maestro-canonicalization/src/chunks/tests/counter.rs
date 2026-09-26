//! Tests of the counting seam: any `TokenCounter` chunks, verified around its batch.
//! Every chunk and prepared-input identity carries its contract ID.
use super::super::identity::{chunk_id, prepared_identity};
use super::*;
use crate::hashing::digest;

/// Documents of the shapes the chunker splits, each under a source path of its own, and a scope
/// that authorizes them all: headings, a nested list holding code, a table cell over the hard
/// maximum, a definition with a footnote, and Unicode with a NUL.
struct Pinned {
    /// The original Markdown of each document.
    markdowns: [String; 5],
    /// Each Markdown canonicalized.
    documents: Vec<CanonicalDocument>,
    /// Authorizes every document's revision.
    scope: DedupScope,
}

impl Pinned {
    fn new() -> Self {
        let markdowns = [
            "# Title\n\nFirst paragraph.\n\n## Section\n\nSecond paragraph with `code`.\n"
                .to_owned(),
            "- first\n  second\n  - nested\n\n    ```rust\n    x\n    y\n    ```\n".to_owned(),
            format!(
                "| Name | Value |\n|---|---|\n| alpha | {} |\n",
                "x ".repeat(800)
            ),
            "Term\n: Definition\n\nText[^note].\n\n[^note]: Detail.\n".to_owned(),
            "# T\r\n\r\nCafé e\u{301} 😀 &amp; \\* literal\0.\r\n".to_owned(),
        ];
        let documents: Vec<_> = markdowns
            .iter()
            .enumerate()
            .map(|(index, markdown)| {
                let path = format!("pinned-{index}");
                canonicalize(CanonicalizeInput::new(markdown, &path)).unwrap()
            })
            .collect();
        let scope = scope(&documents.iter().collect::<Vec<_>>());
        Self {
            markdowns,
            documents,
            scope,
        }
    }

    /// One batch of every pinned document, counted by `counter`.
    fn chunk(&self, counter: &TestCounter) -> Result<ChunkBatch<'_>, Error> {
        let inputs: Vec<_> = self
            .documents
            .iter()
            .zip(&self.markdowns)
            .map(|(document, markdown)| DedupInput { document, markdown })
            .collect();
        chunk_documents(&self.scope, &inputs, WarningPolicy::Preserve, counter)
    }
}

/// Why the pinned batch is refused when the counter's `verify` fails at `call`, counted from one,
/// and how many inputs the counter had tokenized by then.
fn refused_at_verify(call: usize) -> (String, usize) {
    let counter = TestCounter {
        failing_verify: Some(call),
        ..TestCounter::new("test/unqualified")
    };
    let refused = Pinned::new().chunk(&counter).unwrap_err();
    (refused.to_string(), counter.tokenizations.get())
}

#[test]
fn a_test_counter_gives_the_batch_the_helper_gave() {
    let counter = TestCounter::new("test/unqualified");
    let pinned = Pinned::new();
    let batch = pinned.chunk(&counter).unwrap();
    // The batch the test helper `chunk_with_count` built from these documents, with the same
    // count and contract ID, before this seam replaced it: every byte, chunk IDs included.
    assert_eq!(batch.chunks.len(), 15);
    assert_eq!(
        digest(&serde_json::to_vec(&batch).unwrap()),
        "633829b99906c95c01ebba4ea3216ec0781f2ba8622f9bf6509ddff1c67647eb"
    );
    assert_eq!(counter.verifications.get(), 2);
}

#[test]
fn a_verify_failing_after_the_build_refuses_the_batch() {
    let (refusal, tokenizations) = refused_at_verify(2);
    assert_eq!(refusal, "test counter artifacts changed");
    assert!(tokenizations > 0);
}

#[test]
fn a_verify_failing_before_the_build_counts_nothing() {
    let (refusal, tokenizations) = refused_at_verify(1);
    assert_eq!(refusal, "test counter artifacts changed");
    assert_eq!(tokenizations, 0);
}

#[test]
fn every_chunk_identity_carries_the_counters_contract_id() {
    let pinned = Pinned::new();
    for contract_id in ["test/first", "test/second"] {
        let batch = pinned.chunk(&TestCounter::new(contract_id)).unwrap();
        assert_eq!(batch.tokenizer_contract_id, contract_id);
        for chunk in &batch.chunks {
            let occurrence = &batch.deduplication.occurrences[chunk.occurrence_index];
            let mapped = &batch.documents[chunk.occurrence_index].mapped;
            let expected = chunk_id(
                &batch.deduplication,
                occurrence.document,
                mapped,
                &chunk.content,
                contract_id,
            );
            assert_eq!(expected.unwrap(), chunk.chunk_id);
            let prepared = prepared_identity(
                &batch.deduplication,
                contract_id,
                &chunk.content.prepared_input,
            );
            assert_eq!(
                prepared.unwrap().fingerprint,
                chunk.retrieval_input_fingerprint
            );
        }
    }
}
