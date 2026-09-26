//! Explicit local acceptance: never treat an ignored native test as a pass.
#![cfg(test)]
use maestro_canonicalization::{
    CanonicalDocument, CanonicalizeInput, ChunkBatch, DedupInput, DedupScope, InputRole,
    NativeTokenizer, RetrievalChunk, RevisionKey, SplitKind, TokenCounter, WarningPolicy,
    canonicalize, chunk_documents,
};
use serde::Deserialize;
use std::iter;

#[test]
#[ignore = "requires the qualified local GGUF and counter; run explicitly"]
fn public_chunk_example_uses_native_complete_input() {
    let markdown = "Hello world";
    let document = canonicalize(CanonicalizeInput::new(markdown, "example")).unwrap();
    let scope = DedupScope {
        tenant_id: "example-tenant".into(),
        workspace_id: "example-workspace".into(),
        authorized_revisions: [RevisionKey {
            document_id: document.document_id.clone(),
            revision_id: document.revision_id.clone(),
        }]
        .into_iter()
        .collect(),
    };
    let tokenizer = NativeTokenizer::open().unwrap();
    let batch = chunk_documents(
        &scope,
        &[DedupInput {
            document: &document,
            markdown,
        }],
        WarningPolicy::Preserve,
        &tokenizer,
    )
    .unwrap();
    assert_eq!(batch.chunks.len(), 1);
    assert_eq!(batch.chunks[0].content.prepared_input, "Hello world");
    assert_eq!(batch.chunks[0].content.token_count, 4);
    assert_eq!(
        tokenizer
            .token_ids(&batch.chunks[0].content.prepared_input)
            .unwrap(),
        [0, 35378, 8999, 2]
    );
}

fn check_native_batch(batch: &ChunkBatch<'_>, counter: &NativeTokenizer) {
    for chunk in &batch.chunks {
        let prepared: String = chunk
            .content
            .input_parts
            .iter()
            .map(|part| part.text.as_str())
            .collect();
        assert_eq!(prepared, chunk.content.prepared_input);
        assert_eq!(
            counter.token_ids(&prepared).unwrap().len(),
            chunk.content.token_count
        );
        assert!(chunk.content.token_count <= 700);
    }
    for document in &batch.documents {
        for (index, unit) in document
            .mapped
            .units
            .iter()
            .enumerate()
            .filter(|(_, unit)| unit.primary)
        {
            let mut end = 0;
            let mut recovered = String::new();
            for fragment in batch
                .chunks
                .iter()
                .filter(|chunk| chunk.occurrence_index == document.occurrence_index)
                .flat_map(|chunk| &chunk.content.fragments)
                .filter(|fragment| fragment.contribution.unit_index == index)
            {
                let range = fragment.contribution.range;
                assert_eq!(range.start, end);
                recovered.push_str(&unit.text[range.start..range.end]);
                end = range.end;
            }
            assert_eq!(end, unit.text.len());
            assert_eq!(recovered, unit.text);
        }
    }
}

#[test]
#[ignore = "requires the qualified local GGUF and counter; run explicitly"]
fn native_structures_boundaries_and_zero_overlap() {
    let tokenizer = NativeTokenizer::open().unwrap();
    let cases = native_cases(&tokenizer);
    let documents: Vec<_> = cases
        .iter()
        .map(|(name, text)| canonicalize(CanonicalizeInput::new(text, name)).unwrap())
        .collect();
    let scope = native_scope(&documents);
    let inputs: Vec<_> = documents
        .iter()
        .zip(&cases)
        .map(|(document, (_, markdown))| DedupInput { document, markdown })
        .collect();
    let batch = chunk_documents(&scope, &inputs, WarningPolicy::Preserve, &tokenizer).unwrap();
    check_native_batch(&batch, &tokenizer);
    for ((name, _), document) in cases.iter().zip(&documents) {
        let index = occurrence_of(&batch, document);
        let chunks: Vec<_> = batch
            .chunks
            .iter()
            .filter(|chunk| chunk.occurrence_index == index)
            .collect();
        check_plain_and_context_counts(name, &chunks);
        if name == "code" {
            check_code_splits_at_lines(&chunks);
        }
        if name == "table" {
            check_table_rows_keep_their_header(&chunks);
        }
        if name == "list" {
            check_list_items_repeat_their_parent(&chunks);
        }
        if name.starts_with("task-") {
            check_task_status_repeats_on_every_continuation(name, &chunks);
        }
        if name == "deletion" {
            check_deletion_wraps_every_continuation(&batch, index, &chunks);
        }
    }
    check_context_changes_the_prepared_fingerprint(&cases, &documents, &batch);
    println!(
        "native structured acceptance: {} documents, {} chunks, all independently recounted",
        documents.len(),
        batch.chunks.len()
    );
    tokenizer.verify_artifacts().unwrap();
}

/// Plain and heading-context inputs at and around the 500 and 700 token
/// limits, then the structures that force splits.
fn native_cases(tokenizer: &NativeTokenizer) -> Vec<(String, String)> {
    let prefix = "Control-M 9.0.22\n\nServer / SSL\n\n";
    let overhead = tokenizer.token_ids(prefix).unwrap().len();
    let mut cases = Vec::new();
    for expected in [499, 500, 501, 699, 700, 701] {
        cases.push((format!("plain-{expected}"), "a ".repeat(expected - 2)));
        let body = "a ".repeat(expected - overhead);
        assert_eq!(
            tokenizer
                .token_ids(&format!("{prefix}{body}"))
                .unwrap()
                .len(),
            expected
        );
        cases.push((
            format!("context-{expected}"),
            format!("# Control-M 9.0.22\n\n## Server / SSL\n\n{body}"),
        ));
    }
    cases.extend([
        (
            "list".into(),
            format!("- Parent\n  - {}\n", "a ".repeat(1500)),
        ),
        (
            "code".into(),
            format!("```rust linenos=1\n{}\n```\n", "a ".repeat(1500)),
        ),
        (
            "table".into(),
            format!(
                "| Name | Value |\n|---|---|\n| alpha | {} |\n",
                "a ".repeat(1500)
            ),
        ),
        (
            "unicode".into(),
            "# Unicode\r\n\r\nCafé e\u{301} 😀 &amp; \\* literal\0 <mask>.\r\n".into(),
        ),
        ("deletion".into(), format!("~~{}a~~\n", "a ".repeat(1499))),
        (
            "task-checked".into(),
            format!("- [x] {}\n", "a ".repeat(1500)),
        ),
        (
            "task-unchecked".into(),
            format!("- [ ] {}\n", "a ".repeat(1500)),
        ),
        ("context-a".into(), "# First\n\nSame body.\n".into()),
        ("context-b".into(), "# Second\n\nSame body.\n".into()),
    ]);
    cases
}

/// A scope that authorizes exactly these documents' revisions.
fn native_scope(documents: &[CanonicalDocument]) -> DedupScope {
    DedupScope {
        tenant_id: "native-tenant".into(),
        workspace_id: "native-workspace".into(),
        authorized_revisions: documents
            .iter()
            .map(|document| RevisionKey {
                document_id: document.document_id.clone(),
                revision_id: document.revision_id.clone(),
            })
            .collect(),
    }
}

/// The batch occurrence of a document.
fn occurrence_of(batch: &ChunkBatch<'_>, document: &CanonicalDocument) -> usize {
    batch
        .deduplication
        .occurrences
        .iter()
        .position(|occurrence| occurrence.document.document_id == document.document_id)
        .unwrap()
}

/// Up to 700 tokens a plain input is one chunk and a heading-context input
/// two, with exactly the expected count; above, they split further.
fn check_plain_and_context_counts(name: &str, chunks: &[&RetrievalChunk]) {
    if let Some(expected) = name
        .strip_prefix("plain-")
        .and_then(|count| count.parse::<usize>().ok())
    {
        if expected <= 700 {
            assert_eq!(chunks.len(), 1);
            assert_eq!(chunks[0].content.token_count, expected);
        } else {
            assert!(chunks.len() > 1);
        }
    }
    if let Some(expected) = name
        .strip_prefix("context-")
        .and_then(|count| count.parse::<usize>().ok())
    {
        if expected <= 700 {
            assert_eq!(chunks.len(), 2);
            assert_eq!(chunks[1].content.token_count, expected);
        } else {
            assert!(chunks.len() > 2);
        }
    }
}

/// Oversized code splits at line fragments.
fn check_code_splits_at_lines(chunks: &[&RetrievalChunk]) {
    assert!(chunks.iter().any(|chunk| {
        chunk
            .content
            .fragments
            .iter()
            .any(|fragment| fragment.split == SplitKind::CodeLineFragment)
    }));
}

/// An oversized cell keeps its column window and repeats its column header.
fn check_table_rows_keep_their_header(chunks: &[&RetrievalChunk]) {
    assert!(chunks.iter().any(|chunk| {
        chunk
            .content
            .table_windows
            .iter()
            .any(|window| window.columns == [1])
    }));
    assert!(chunks.iter().any(|chunk| {
        chunk
            .content
            .input_parts
            .iter()
            .any(|part| part.role == InputRole::TableHeaderContext && part.text == "Value")
    }));
}

/// A nested item's continuations repeat the parent item's text.
fn check_list_items_repeat_their_parent(chunks: &[&RetrievalChunk]) {
    assert!(chunks.iter().any(|chunk| {
        chunk
            .content
            .input_parts
            .iter()
            .any(|part| part.role == InputRole::ParentListContext && part.text == "Parent")
    }));
}

/// Every continuation of a task item repeats its source-backed status.
fn check_task_status_repeats_on_every_continuation(name: &str, chunks: &[&RetrievalChunk]) {
    let status = if name == "task-checked" {
        "[x] "
    } else {
        "[ ] "
    };
    for chunk in chunks
        .iter()
        .filter(|chunk| chunk.content.body_text.contains('a'))
    {
        assert!(
            chunk
                .content
                .input_parts
                .iter()
                .any(|part| part.role == InputRole::StructuralContext
                    && part.text == status
                    && !part.contributions.is_empty())
        );
    }
}

/// A split deletion keeps its envelope, and every continuation is wrapped.
fn check_deletion_wraps_every_continuation(
    batch: &ChunkBatch<'_>,
    index: usize,
    chunks: &[&RetrievalChunk],
) {
    assert!(
        batch.documents[index]
            .mapped
            .units
            .iter()
            .any(|unit| !unit.envelopes.is_empty())
    );
    assert!(
        chunks
            .iter()
            .all(|chunk| chunk.content.prepared_input.starts_with("~~")
                && chunk.content.prepared_input.ends_with("~~"))
    );
}

/// The same body under different headings gets different prepared-input
/// fingerprints.
fn check_context_changes_the_prepared_fingerprint(
    cases: &[(String, String)],
    documents: &[CanonicalDocument],
    batch: &ChunkBatch<'_>,
) {
    let contextual: Vec<_> = cases
        .iter()
        .zip(documents)
        .filter(|((name, _), _)| name == "context-a" || name == "context-b")
        .map(|(_, document)| occurrence_of(batch, document))
        .collect();
    let first = batch
        .chunks
        .iter()
        .find(|chunk| chunk.occurrence_index == contextual[0])
        .unwrap();
    let second = batch
        .chunks
        .iter()
        .find(|chunk| chunk.occurrence_index == contextual[1])
        .unwrap();
    assert_ne!(
        first.retrieval_input_fingerprint,
        second.retrieval_input_fingerprint
    );
}

#[test]
#[ignore = "requires the qualified local GGUF and counter; run explicitly"]
fn native_mandatory_context_is_not_clipped() {
    let tokenizer = NativeTokenizer::open().unwrap();
    for markdown in [
        format!("# {}\n\nbody\n", "a ".repeat(800)),
        format!("- {}\n  - child\n", "a ".repeat(800)),
    ] {
        let document = canonicalize(CanonicalizeInput::new(&markdown, "context-blocker")).unwrap();
        let scope = DedupScope {
            tenant_id: "native-tenant".into(),
            workspace_id: "native-workspace".into(),
            authorized_revisions: [RevisionKey {
                document_id: document.document_id.clone(),
                revision_id: document.revision_id.clone(),
            }]
            .into_iter()
            .collect(),
        };
        let error = chunk_documents(
            &scope,
            &[DedupInput {
                document: &document,
                markdown: &markdown,
            }],
            WarningPolicy::Preserve,
            &tokenizer,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("does not fit in 700 tokens with its context"),
            "unexpected native refusal: {error}"
        );
    }
    tokenizer.verify_artifacts().unwrap();
}

/// The native profile's parity fixtures, which maestro-knowledge's router
/// tokenizer must match to qualify: the 41 complete inputs of the S0
/// qualification, the eleven that named a vendor made neutral, each with the
/// ordered IDs this counter gave it. This counter alone records them.
const PARITY: &str = include_str!("../../../maestro-knowledge/src/prepare/native-parity.json");

/// The parity fixtures' file.
#[derive(Deserialize)]
struct Parity {
    /// The native contract ID the IDs were recorded under.
    profile: String,
    /// The fixtures, in their order.
    fixtures: Vec<ParityFixture>,
}

/// A parity fixture as the file writes it.
#[derive(Deserialize)]
struct ParityFixture {
    /// The name a failure gives.
    name: String,
    /// The parts of the complete input.
    input: Vec<Run<String>>,
    /// The runs of the ordered IDs.
    ids: Vec<Run<u32>>,
}

/// A part of an input or a run of IDs: one item, or one item repeated.
#[derive(Deserialize)]
#[serde(untagged)]
enum Run<T> {
    /// The item once.
    Once(T),
    /// `repeat`, `times` times.
    Repeated {
        /// The item.
        repeat: T,
        /// How many times.
        times: usize,
    },
}

/// Each run's item, as many times as the run repeats it.
fn expand<T: Clone>(runs: Vec<Run<T>>) -> impl Iterator<Item = T> {
    runs.into_iter().flat_map(|run| match run {
        Run::Once(item) => iter::repeat_n(item, 1),
        Run::Repeated { repeat, times } => iter::repeat_n(repeat, times),
    })
}

/// Every parity fixture, the budgets' and the context's boundaries among
/// them, gets its recorded IDs, twice, under the recorded contract ID.
#[test]
#[ignore = "requires the qualified local GGUF and counter; run explicitly"]
fn qualified_native_ids_and_boundaries() {
    let counter = NativeTokenizer::open().unwrap();
    let parity: Parity = serde_json::from_str(PARITY).unwrap();
    assert_eq!(parity.profile, counter.contract_id());
    assert_eq!(parity.fixtures.len(), 41);
    for fixture in parity.fixtures {
        let input: String = expand(fixture.input).collect();
        let expected: Vec<u32> = expand(fixture.ids).collect();
        assert_eq!(
            counter.token_ids(&input).unwrap(),
            expected,
            "{}",
            fixture.name
        );
        assert_eq!(
            counter.token_ids(&input).unwrap(),
            expected,
            "{}",
            fixture.name
        );
    }
    counter.verify_artifacts().unwrap();
}
