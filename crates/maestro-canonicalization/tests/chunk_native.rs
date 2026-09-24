//! Explicit local acceptance: never treat an ignored native test as a pass.
#![cfg(test)]
use maestro_canonicalization::{
    CanonicalDocument, CanonicalizeInput, ChunkBatch, DedupInput, DedupScope, InputRole,
    NativeTokenizer, RetrievalChunk, RevisionKey, SplitKind, WarningPolicy, canonicalize,
    chunk_documents,
};

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

fn check_native_batch(batch: &maestro_canonicalization::ChunkBatch<'_>, counter: &NativeTokenizer) {
    for chunk in &batch.chunks {
        let prepared: String = chunk
            .content
            .input_parts
            .iter()
            .map(|p| p.text.as_str())
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
                .filter(|f| f.contribution.unit_index == index)
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
            .filter(|c| c.occurrence_index == index)
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
            .map(|d| RevisionKey {
                document_id: d.document_id.clone(),
                revision_id: d.revision_id.clone(),
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
        .position(|o| o.document.document_id == document.document_id)
        .unwrap()
}

/// Up to 700 tokens a plain input is one chunk and a heading-context input
/// two, with exactly the expected count; above, they split further.
fn check_plain_and_context_counts(name: &str, chunks: &[&RetrievalChunk]) {
    if let Some(expected) = name
        .strip_prefix("plain-")
        .and_then(|s| s.parse::<usize>().ok())
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
        .and_then(|s| s.parse::<usize>().ok())
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
    assert!(chunks.iter().any(|c| {
        c.content
            .fragments
            .iter()
            .any(|f| f.split == SplitKind::CodeLineFragment)
    }));
}

/// An oversized cell keeps its column window and repeats its column header.
fn check_table_rows_keep_their_header(chunks: &[&RetrievalChunk]) {
    assert!(
        chunks
            .iter()
            .any(|c| c.content.table_windows.iter().any(|w| w.columns == [1]))
    );
    assert!(chunks.iter().any(|c| {
        c.content
            .input_parts
            .iter()
            .any(|p| p.role == InputRole::TableHeaderContext && p.text == "Value")
    }));
}

/// A nested item's continuations repeat the parent item's text.
fn check_list_items_repeat_their_parent(chunks: &[&RetrievalChunk]) {
    assert!(chunks.iter().any(|c| {
        c.content
            .input_parts
            .iter()
            .any(|p| p.role == InputRole::ParentListContext && p.text == "Parent")
    }));
}

/// Every continuation of a task item repeats its source-backed status.
fn check_task_status_repeats_on_every_continuation(name: &str, chunks: &[&RetrievalChunk]) {
    let status = if name == "task-checked" {
        "[x] "
    } else {
        "[ ] "
    };
    for chunk in chunks.iter().filter(|c| c.content.body_text.contains('a')) {
        assert!(
            chunk
                .content
                .input_parts
                .iter()
                .any(|p| p.role == InputRole::StructuralContext
                    && p.text == status
                    && !p.contributions.is_empty())
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
            .all(|c| c.content.prepared_input.starts_with("~~")
                && c.content.prepared_input.ends_with("~~"))
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
    let a = batch
        .chunks
        .iter()
        .find(|c| c.occurrence_index == contextual[0])
        .unwrap();
    let b = batch
        .chunks
        .iter()
        .find(|c| c.occurrence_index == contextual[1])
        .unwrap();
    assert_ne!(a.retrieval_input_fingerprint, b.retrieval_input_fingerprint);
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
            error.to_string().contains("chunk context/structure"),
            "unexpected native refusal: {error}"
        );
    }
    tokenizer.verify_artifacts().unwrap();
}

#[test]
#[ignore = "requires the qualified local GGUF and counter; run explicitly"]
fn qualified_native_ids_and_boundaries() {
    let counter = NativeTokenizer::open().unwrap();
    assert_eq!(
        counter.token_ids("Hello world").unwrap(),
        [0, 35378, 8999, 2]
    );
    assert_eq!(counter.token_ids("").unwrap(), [0, 2]);
    for expected in [499, 500, 501, 699, 700, 701, 8192, 8193] {
        let input = "a ".repeat(expected - 2);
        assert_eq!(counter.token_ids(&input).unwrap().len(), expected);
    }
    let prefix = "# Control-M 9.0.22\n\n## Server / SSL\n\n";
    let overhead = counter.token_ids(prefix).unwrap().len();
    for expected in [499, 500, 501, 699, 700, 701] {
        assert!(overhead < expected);
        let input = format!("{prefix}{}", "a ".repeat(expected - overhead));
        assert_eq!(counter.token_ids(&input).unwrap().len(), expected);
    }
    for (input, expected) in [("  a   b  ", 4), ("left   <mask>   right", 8)] {
        assert_eq!(counter.token_ids(input).unwrap().len(), expected);
    }
    // Independent ordered-ID goldens captured by the approved #19 qualification.
    for (input, expected) in [
        ("a\0b", vec![0, 10, 3, 275, 2]),
        (
            r"path\new\table\u00e9",
            vec![
                0, 60875, 41872, 54936, 41872, 22819, 41872, 34, 7049, 13, 1126, 2,
            ],
        ),
        (
            "café naïve Ångström",
            vec![0, 26216, 24, 9392, 272, 8839, 449, 30011, 2],
        ),
        ("<s></s>", vec![0, 0, 2, 2]),
    ] {
        assert_eq!(counter.token_ids(input).unwrap(), expected);
        assert_eq!(counter.token_ids(input).unwrap(), expected);
    }
    counter.verify_artifacts().unwrap();
}
