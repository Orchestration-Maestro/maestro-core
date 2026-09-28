//! Locating a quote before a claim cites it (FR-S2-003): the canonical
//! document is the revision's own and its Markdown the rule's source, the
//! row belongs to its table and every cell to its row, the row's one span is
//! nonempty, on character boundaries and inside its table's, and each cell's
//! text is the source's own bytes, never normalized or rendered text. The
//! kernel then checks the revision, its digest and the quote's bytes again,
//! independently, when it admits the claim.

use maestro_canonicalization::{Block, CanonicalDocument, SourceSpan};
use maestro_kernel::{
    artifact::Digest,
    document::Revision,
    evidence::Span,
    facts::Support,
    store::{self, Database},
};
use std::{error, fmt};

/// A revision to extract from: its id, its canonical document and its
/// original Markdown.
#[derive(Debug, Clone)]
pub struct Source {
    /// The revision.
    revision_id: String,
    /// Its canonical document.
    canonical: CanonicalDocument,
    /// Its original Markdown.
    markdown: String,
}

/// Why a revision's artifacts cannot be read as a [`Source`].
#[derive(Debug)]
pub enum SourceError {
    /// The artifact store refused.
    Store(store::Error),
    /// Its canonical artifact is not a canonical document.
    Canonical(serde_json::Error),
    /// Its original is not UTF-8.
    NotUtf8,
}

/// A located row: the support it gives a claim and its cells' text, read
/// from the original bytes.
#[derive(Debug)]
pub(super) struct Located<'s> {
    /// Where the row lies, and the digest of its bytes.
    pub(super) support: Support,
    /// Each cell's source text, without the spaces around it.
    pub(super) cells: Vec<&'s str>,
}

impl Source {
    /// The revision `revision_id`, of `canonical` and `markdown`.
    #[must_use]
    pub fn new(revision_id: String, canonical: CanonicalDocument, markdown: String) -> Self {
        Self {
            revision_id,
            canonical,
            markdown,
        }
    }

    /// `revision` with the canonical document and the original its
    /// artifacts hold.
    ///
    /// # Errors
    ///
    /// [`SourceError::Store`] when an artifact cannot be read,
    /// [`SourceError::Canonical`] and [`SourceError::NotUtf8`] when one is
    /// not what the revision says.
    pub fn read(database: &Database, revision: &Revision) -> Result<Self, SourceError> {
        let canonical = database
            .get(&revision.canonical_digest)
            .map_err(SourceError::Store)?;
        let canonical = serde_json::from_slice(&canonical).map_err(SourceError::Canonical)?;
        let original = database
            .get(&revision.original_digest)
            .map_err(SourceError::Store)?;
        let markdown = String::from_utf8(original).map_err(|_| SourceError::NotUtf8)?;
        Ok(Self::new(revision.id.clone(), canonical, markdown))
    }

    /// The revision's id.
    #[must_use]
    pub fn revision_id(&self) -> &str {
        &self.revision_id
    }

    /// Its canonical document.
    #[must_use]
    pub fn canonical(&self) -> &CanonicalDocument {
        &self.canonical
    }

    /// Its original Markdown.
    #[must_use]
    pub fn markdown(&self) -> &str {
        &self.markdown
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => fmt::Display::fmt(error, formatter),
            Self::Canonical(error) => {
                write!(formatter, "its canonical document cannot be read: {error}")
            }
            Self::NotUtf8 => formatter.write_str("its original is not UTF-8"),
        }
    }
}

impl error::Error for SourceError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Canonical(error) => Some(error),
            Self::NotUtf8 => None,
        }
    }
}

/// Checks that `source`'s canonical document is its revision's and was made
/// from its Markdown, and that the Markdown's digest is `digest`, the one a
/// rule is bound to.
///
/// # Errors
///
/// Why it is not, for people.
pub(super) fn check_source(source: &Source, digest: &Digest) -> Result<(), String> {
    let canonical = &source.canonical;
    if canonical.revision_id != source.revision_id {
        return Err(format!(
            "the canonical document is another revision's, {}",
            canonical.revision_id
        ));
    }
    let found = Digest::of(source.markdown.as_bytes());
    if canonical.content_hash != format!("sha256:{}", found.as_str()) {
        return Err("the canonical document was made from other Markdown".to_owned());
    }
    if found != *digest {
        return Err(format!(
            "the source digest sha256:{} is not the rule's sha256:{}",
            found.as_str(),
            digest.as_str()
        ));
    }
    Ok(())
}

/// Locates the row `row` of the table `table`, with its cells `cells`, in
/// `source`'s original Markdown.
///
/// # Errors
///
/// Why its quote is refused, for people.
pub(super) fn locate<'s>(
    source: &'s Source,
    table: &Block,
    row: &Block,
    cells: &[&Block],
) -> Result<Located<'s>, String> {
    let markdown = source.markdown.as_str();
    if row.parent_block_id.as_ref() != Some(&table.block_id) {
        return Err(format!(
            "the block {} is not a row of its table",
            row.block_id
        ));
    }
    for block in [table, row].into_iter().chain(cells.iter().copied()) {
        if block.revision_id != source.revision_id {
            return Err(format!(
                "the block {} is another revision's",
                block.block_id
            ));
        }
    }
    let span = one_span(row, markdown)?;
    if !table.source_spans.iter().any(|outer| within(span, *outer)) {
        return Err(format!("the row {} lies outside its table", row.block_id));
    }
    let mut texts = Vec::with_capacity(cells.len());
    let mut outside = span.start;
    for cell in cells {
        if cell.parent_block_id.as_ref() != Some(&row.block_id) {
            return Err(format!(
                "the block {} is not a cell of its row",
                cell.block_id
            ));
        }
        let cell_span = one_span(cell, markdown)?;
        if !within(cell_span, span) || cell_span.start < outside {
            return Err(format!("the cell {} lies outside its row", cell.block_id));
        }
        separators(markdown, row, outside, cell_span.start)?;
        outside = cell_span.end;
        let text = markdown
            .get(cell_span.start..cell_span.end)
            .unwrap_or_default()
            .trim_matches([' ', '\t']);
        if text != cell.retrieval_text {
            return Err(format!(
                "the cell {} reads {:?}, not the source's {text:?}",
                cell.block_id, cell.retrieval_text
            ));
        }
        texts.push(text);
    }
    separators(markdown, row, outside, span.end)?;
    let quote = markdown.get(span.start..span.end).unwrap_or_default();
    Ok(Located {
        support: Support {
            revision_id: source.revision_id.clone(),
            block_id: row.block_id.clone(),
            span: Span {
                start: span.start,
                end: span.end,
            },
            quote_digest: Digest::of(quote.as_bytes()),
        },
        cells: texts,
    })
}

/// The one source span of `block`, once it is nonempty and on character
/// boundaries of `markdown`.
fn one_span(block: &Block, markdown: &str) -> Result<SourceSpan, String> {
    let [span] = block.source_spans.as_slice() else {
        return Err(format!(
            "the block {} is ambiguous: it has {} source spans",
            block.block_id,
            block.source_spans.len()
        ));
    };
    if !span.is_valid(markdown) {
        return Err(format!(
            "the span {}..{} of the block {} is not on UTF-8 boundaries of its source",
            span.start, span.end, block.block_id
        ));
    }
    if span.start == span.end {
        return Err(format!("the span of the block {} is empty", block.block_id));
    }
    Ok(*span)
}

/// Checks that the bytes of `markdown` from `start` to `end`, between cells
/// of `row`, are only the table's pipes and blanks: a row says nothing its
/// cells do not.
fn separators(markdown: &str, row: &Block, start: usize, end: usize) -> Result<(), String> {
    let between = markdown.get(start..end).unwrap_or_default();
    if between
        .chars()
        .all(|character| matches!(character, '|' | ' ' | '\t' | '\r' | '\n'))
    {
        Ok(())
    } else {
        Err(format!(
            "the row {} holds {between:?} outside its cells",
            row.block_id
        ))
    }
}

/// Whether `inner` lies inside `outer`.
fn within(inner: SourceSpan, outer: SourceSpan) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}
