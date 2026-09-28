//! Locating a row's quote: the row belongs to its table and its cells to the
//! row, its one span lies inside the table's on character boundaries, and
//! every cell's text is the source's own bytes; the canonical document is
//! the revision's own.

use super::support::{markdown, source};
use crate::graph::{
    structure::tables,
    verify::{Source, SourceError, check_source, locate},
};
use maestro_canonicalization::{Block, CanonicalDocument, SourceSpan};
use maestro_kernel::{
    artifact::Digest,
    document::{Revision, RevisionStatus},
    evidence::Span,
    store::Database,
};
use serde_json::Map;
use std::{
    env, error, fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// The frozen source's canonical document.
fn canonical() -> CanonicalDocument {
    source(&markdown()).canonical().clone()
}

/// The ids of the one table of `document`, of its body row `index` and of
/// that row's cells.
fn ids(document: &CanonicalDocument, index: usize) -> (String, String, Vec<String>) {
    let [table] = tables(document)
        .try_into()
        .unwrap_or_else(|_| panic!("one table"));
    let row = &table.rows[index + 1];
    (
        table.block.block_id.clone(),
        row.block.block_id.clone(),
        row.cells.iter().map(|cell| cell.block_id.clone()).collect(),
    )
}

/// The block `id` of `document`.
fn block<'d>(document: &'d CanonicalDocument, id: &str) -> &'d Block {
    document
        .blocks
        .iter()
        .find(|block| block.block_id == id)
        .unwrap()
}

/// Changes, in a copy of the frozen canonical document, body row `index`
/// (its first cell when `cell`) with `change`, and returns why that row is
/// refused then.
fn refused_after(index: usize, cell: bool, change: fn(&mut Block)) -> String {
    let mut document = canonical();
    let (table, row, cells) = ids(&document, index);
    let changed = if cell { &cells[0] } else { &row };
    let at = document
        .blocks
        .iter()
        .position(|block| block.block_id == *changed)
        .unwrap();
    change(&mut document.blocks[at]);
    let source = Source::new(document.revision_id.clone(), document, markdown());
    let document = source.canonical();
    let cells: Vec<&Block> = cells.iter().map(|id| block(document, id)).collect();
    locate(
        &source,
        block(document, &table),
        block(document, &row),
        &cells,
    )
    .unwrap_err()
}

#[test]
fn every_row_quote_is_the_original_bytes() {
    let markdown = markdown();
    let source = source(&markdown);
    let [table] = tables(source.canonical())
        .try_into()
        .unwrap_or_else(|_| panic!("one table"));
    let mut located = 0;
    for row in &table.rows {
        let found = locate(&source, table.block, row.block, &row.cells).unwrap();
        let [span] = row.block.source_spans.as_slice() else {
            panic!("one span");
        };
        let quote = &markdown.as_bytes()[span.start..span.end];
        assert_eq!(
            found.support.span,
            Span {
                start: span.start,
                end: span.end
            }
        );
        assert_eq!(found.support.block_id, row.block.block_id);
        assert_eq!(found.support.revision_id, source.revision_id());
        assert_eq!(found.support.quote_digest, Digest::of(quote));
        for (cell, text) in row.cells.iter().zip(&found.cells) {
            assert_eq!(*text, cell.retrieval_text);
        }
        located += 1;
    }
    assert_eq!(located, 5);
}

#[test]
fn a_row_with_two_spans_is_ambiguous() {
    let refused = refused_after(0, false, |block| {
        let span = block.source_spans[0];
        block.source_spans.push(span);
    });
    assert!(refused.contains("ambiguous"), "{refused}");
}

#[test]
fn a_row_span_inside_a_character_is_refused() {
    let refused = refused_after(0, false, |block| block.source_spans[0].end -= 4);
    assert!(refused.contains("UTF-8"), "{refused}");
}

#[test]
fn an_empty_row_span_is_refused() {
    let refused = refused_after(0, false, |block| {
        block.source_spans[0].end = block.source_spans[0].start;
    });
    assert!(refused.contains("empty"), "{refused}");
}

#[test]
fn a_row_span_outside_its_table_is_refused() {
    let refused = refused_after(0, false, |block| {
        block.source_spans[0] = SourceSpan { start: 0, end: 21 };
    });
    assert!(refused.contains("outside its table"), "{refused}");
}

#[test]
fn a_row_of_another_block_is_refused() {
    let refused = refused_after(1, false, |block| {
        block.parent_block_id = Some("block-other".to_owned());
    });
    assert!(refused.contains("not a row of"), "{refused}");
}

#[test]
fn a_cell_of_another_row_is_refused() {
    let refused = refused_after(1, true, |block| {
        block.parent_block_id = Some("block-other".to_owned());
    });
    assert!(refused.contains("not a cell of"), "{refused}");
}

#[test]
fn a_cell_span_outside_its_row_is_refused() {
    let refused = refused_after(1, true, |block| {
        block.source_spans[0] = SourceSpan { start: 0, end: 21 };
    });
    assert!(refused.contains("outside its row"), "{refused}");
}

#[test]
fn a_cell_with_two_spans_is_ambiguous() {
    let refused = refused_after(1, true, |block| {
        let span = block.source_spans[0];
        block.source_spans.push(span);
    });
    assert!(refused.contains("ambiguous"), "{refused}");
}

#[test]
fn normalized_cell_text_is_not_the_source() {
    let refused = refused_after(0, true, |block| {
        block.retrieval_text = "LABEL".to_owned();
    });
    assert!(refused.contains("not the source"), "{refused}");
}

#[test]
fn a_block_of_another_revision_is_refused() {
    let refused = refused_after(2, false, |block| {
        block.revision_id = "rev-other".to_owned();
    });
    assert!(refused.contains("another revision"), "{refused}");
}

#[test]
fn the_canonical_document_must_be_the_revisions_own() {
    let markdown = markdown();
    let digest = Digest::of(markdown.as_bytes());
    let own = source(&markdown);
    assert_eq!(check_source(&own, &digest), Ok(()));
    let other = Source::new("rev-other".to_owned(), canonical(), markdown.clone());
    let refused = check_source(&other, &digest).unwrap_err();
    assert!(refused.contains("another revision"), "{refused}");
    let changed = markdown.replace("café", "cafe");
    let stale = Source::new(own.revision_id().to_owned(), canonical(), changed.clone());
    let refused = check_source(&stale, &Digest::of(changed.as_bytes())).unwrap_err();
    assert!(refused.contains("other Markdown"), "{refused}");
    let refused = check_source(&own, &Digest::of(b"other")).unwrap_err();
    assert!(refused.contains("source digest"), "{refused}");
}

/// A kernel database in a new directory under the platform's temporary
/// directory, removed with it when dropped.
struct Scratch(PathBuf, Database);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-knowledge-graph-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let database = Database::open_in(&path).unwrap();
        Self(path, database)
    }

    /// A revision named as the frozen source's, whose artifacts hold
    /// `canonical` and `original`.
    fn revision(&self, canonical: &[u8], original: &[u8]) -> Revision {
        Revision {
            id: source(&markdown()).revision_id().to_owned(),
            document_id: "doc-graph".to_owned(),
            original_digest: self.1.put(original, "text/markdown").unwrap(),
            canonical_digest: self.1.put(canonical, "application/json").unwrap(),
            status: RevisionStatus::Valid,
            captured_at: None,
            metadata: Map::new(),
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

#[test]
fn a_source_is_read_from_its_revisions_artifacts() {
    let scratch = Scratch::new();
    let markdown = markdown();
    let canonical = serde_json::to_vec(&canonical()).unwrap();
    let revision = scratch.revision(&canonical, markdown.as_bytes());
    let read = Source::read(&scratch.1, &revision).unwrap();
    assert_eq!(read.revision_id(), revision.id);
    assert_eq!(read.canonical(), &self::canonical());
    assert_eq!(read.markdown(), markdown);
}

#[test]
fn a_source_whose_artifacts_are_not_what_they_say_is_refused() {
    let scratch = Scratch::new();
    let canonical = serde_json::to_vec(&canonical()).unwrap();
    let not_canonical = scratch.revision(b"{}", markdown().as_bytes());
    let error = Source::read(&scratch.1, &not_canonical).unwrap_err();
    assert!(matches!(error, SourceError::Canonical(_)), "{error:?}");
    assert!(
        error
            .to_string()
            .contains("canonical document cannot be read")
    );
    assert!(error::Error::source(&error).is_some());
    let not_utf8 = scratch.revision(&canonical, b"label \xe9");
    let error = Source::read(&scratch.1, &not_utf8).unwrap_err();
    assert!(matches!(error, SourceError::NotUtf8), "{error:?}");
    assert_eq!(error.to_string(), "its original is not UTF-8");
    assert!(error::Error::source(&error).is_none());
    let mut missing = scratch.revision(&canonical, markdown().as_bytes());
    missing.canonical_digest = Digest::of(b"never stored");
    let error = Source::read(&scratch.1, &missing).unwrap_err();
    assert!(matches!(error, SourceError::Store(_)), "{error:?}");
    assert!(error::Error::source(&error).is_some());
    assert!(!error.to_string().is_empty());
    let mut missing = scratch.revision(&canonical, markdown().as_bytes());
    missing.original_digest = Digest::of(b"never stored");
    let error = Source::read(&scratch.1, &missing).unwrap_err();
    assert!(matches!(error, SourceError::Store(_)), "{error:?}");
}

#[test]
fn overlapping_cells_are_refused() {
    let mut document = canonical();
    let (table, row, cells) = ids(&document, 1);
    let first = block(&document, &cells[0]).clone();
    let at = document
        .blocks
        .iter()
        .position(|block| block.block_id == cells[1])
        .unwrap();
    document.blocks[at].source_spans = first.source_spans;
    document.blocks[at].retrieval_text = first.retrieval_text;
    let source = Source::new(document.revision_id.clone(), document, markdown());
    let document = source.canonical();
    let cells: Vec<&Block> = cells.iter().map(|id| block(document, id)).collect();
    let refused = locate(
        &source,
        block(document, &table),
        block(document, &row),
        &cells,
    )
    .unwrap_err();
    assert!(refused.contains("outside its row"), "{refused}");
}
