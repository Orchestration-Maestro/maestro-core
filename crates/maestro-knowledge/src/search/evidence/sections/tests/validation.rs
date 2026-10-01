//! The length-only path rejects the same malformed block spans as the text path.

use crate::search::evidence::sections::{index::SectionIndex, validation::block_span_with_length};
use maestro_canonicalization::{Block, CanonicalizeInput, SourceSpan, canonicalize};
use maestro_kernel::evidence::Span;

const MARKDOWN: &str = "## Guide\n\nSource text.\n";

fn source_block() -> Block {
    canonicalize(CanonicalizeInput::new(MARKDOWN, "fixture.md"))
        .unwrap()
        .blocks
        .into_iter()
        .find(|block| !block.source_spans.is_empty())
        .unwrap()
}

fn length_index_with_spans(source_spans: Vec<SourceSpan>) -> Result<SectionIndex, String> {
    let mut document = canonicalize(CanonicalizeInput::new(MARKDOWN, "fixture.md")).unwrap();
    document
        .blocks
        .iter_mut()
        .find(|block| !block.source_spans.is_empty())
        .unwrap()
        .source_spans = source_spans;
    SectionIndex::new_from_length(&document, MARKDOWN.len())
}

#[test]
fn refuses_reversed_and_out_of_range_first_spans() {
    let mut block = source_block();
    let reversed = vec![SourceSpan { start: 2, end: 1 }];
    block.source_spans.clone_from(&reversed);
    assert!(block_span_with_length(&block, MARKDOWN.len()).is_err());
    assert!(length_index_with_spans(reversed).is_err());

    let too_long = vec![SourceSpan {
        start: 0,
        end: MARKDOWN.len() + 1,
    }];
    block.source_spans.clone_from(&too_long);
    assert!(block_span_with_length(&block, MARKDOWN.len()).is_err());
    assert!(length_index_with_spans(too_long).is_err());
}

#[test]
fn accepts_end_at_length_and_empty_spans() {
    let mut block = source_block();
    let at_end = vec![SourceSpan {
        start: 0,
        end: MARKDOWN.len(),
    }];
    block.source_spans.clone_from(&at_end);
    assert_eq!(
        block_span_with_length(&block, MARKDOWN.len()),
        Ok(Some(Span {
            start: 0,
            end: MARKDOWN.len(),
        }))
    );
    assert!(length_index_with_spans(at_end).is_ok());

    let empty = vec![SourceSpan { start: 1, end: 1 }];
    block.source_spans.clone_from(&empty);
    assert_eq!(
        block_span_with_length(&block, MARKDOWN.len()),
        Ok(Some(Span { start: 1, end: 1 }))
    );
    assert!(length_index_with_spans(empty).is_ok());
}

#[test]
fn checks_each_later_span_for_reversal_and_source_length() {
    let mut block = source_block();
    let reversed_later = vec![
        SourceSpan { start: 0, end: 1 },
        SourceSpan { start: 9, end: 8 },
    ];
    block.source_spans.clone_from(&reversed_later);
    assert!(block_span_with_length(&block, MARKDOWN.len()).is_err());
    assert!(length_index_with_spans(reversed_later).is_err());

    let too_long_later = vec![
        SourceSpan { start: 0, end: 1 },
        SourceSpan {
            start: MARKDOWN.len(),
            end: MARKDOWN.len() + 1,
        },
    ];
    block.source_spans.clone_from(&too_long_later);
    assert!(block_span_with_length(&block, MARKDOWN.len()).is_err());
    assert!(length_index_with_spans(too_long_later).is_err());
}
