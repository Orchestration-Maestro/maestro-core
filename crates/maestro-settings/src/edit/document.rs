//! Where a preferences file writes each key and table, from the spans of
//! the TOML parser: what an edit needs to change a value's bytes, remove a
//! key's lines, or add a line in the right table.

use std::ops::Range;
use toml::{
    Spanned,
    de::{DeTable, DeValue},
};

/// A key with a value, as the file writes it.
#[derive(Debug)]
pub(super) struct Entry {
    /// Its dotted path from the root.
    pub(super) path: String,
    /// The span of its last key segment.
    pub(super) key: Range<usize>,
    /// The span of its value.
    pub(super) value: Range<usize>,
}

/// How a table is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Form {
    /// A `[header]`, whose span is the header's.
    Header,
    /// Made by dotted keys, such as `search` in `search.k = 1`.
    Dotted,
    /// An inline `{ ... }` table.
    Inline,
}

/// A table, as the file writes it.
#[derive(Debug)]
pub(super) struct Table {
    /// Its dotted path.
    pub(super) path: String,
    /// How it is written.
    pub(super) form: Form,
    /// The span of its header, for a header table.
    pub(super) span: Range<usize>,
}

/// Every key and table of a file.
#[derive(Debug, Default)]
pub(super) struct Document {
    /// The keys with values, in no particular order.
    pub(super) entries: Vec<Entry>,
    /// The tables, in no particular order.
    pub(super) tables: Vec<Table>,
}

impl Document {
    /// The keys and tables of `root`, a file's parsed text.
    pub(super) fn of(root: &DeTable<'_>, text: &str) -> Self {
        let mut document = Self::default();
        document.walk(root, "", text);
        document
    }

    /// Records the entries and tables of `table`, at `prefix`.
    fn walk(&mut self, table: &DeTable<'_>, prefix: &str, text: &str) {
        for (key, value) in table {
            let path = if prefix.is_empty() {
                key.get_ref().to_string()
            } else {
                format!("{prefix}.{}", key.get_ref())
            };
            match value.get_ref() {
                DeValue::Table(inner) => {
                    self.tables.push(Table {
                        path: path.clone(),
                        form: form(value, key.span(), text),
                        span: value.span(),
                    });
                    self.walk(inner, &path, text);
                }
                _ => self.entries.push(Entry {
                    path,
                    key: key.span(),
                    value: value.span(),
                }),
            }
        }
    }

    /// The entry of `path`.
    pub(super) fn entry(&self, path: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.path == path)
    }

    /// The table of `path`.
    pub(super) fn table(&self, path: &str) -> Option<&Table> {
        self.tables.iter().find(|table| table.path == path)
    }

    /// Where the first header starts, or the end of `text` without one.
    pub(super) fn first_header(&self, text: &str) -> usize {
        self.headers()
            .map(|span| span.start)
            .min()
            .unwrap_or(text.len())
    }

    /// Where the header after `offset` starts, or the end of `text`.
    pub(super) fn next_header(&self, offset: usize, text: &str) -> usize {
        self.headers()
            .map(|span| span.start)
            .filter(|start| *start > offset)
            .min()
            .unwrap_or(text.len())
    }

    /// The spans of every header.
    fn headers(&self) -> impl Iterator<Item = &Range<usize>> {
        self.tables
            .iter()
            .filter(|table| table.form == Form::Header)
            .map(|table| &table.span)
    }
}

/// How the table `value`, of the key at `key`, is written: its span is the
/// header's for a header table, the key's for a dotted one, and the braces'
/// for an inline one.
fn form(value: &Spanned<DeValue<'_>>, key: Range<usize>, text: &str) -> Form {
    let span = value.span();
    if span == key {
        Form::Dotted
    } else if text.get(span).is_some_and(|source| source.starts_with('{')) {
        Form::Inline
    } else {
        Form::Header
    }
}
