//! `config set` and `config unset` on a preferences file's text. A set
//! replaces only the bytes of the value, or adds one line in the key's table
//! (after the table's last line, beside dotted siblings, or under a new
//! header at the end); an unset removes the key's lines. Comments, order,
//! line endings and a missing last newline are kept. The file must be valid
//! before the edit, and the result is parsed strictly and compared with the
//! intended change before it is returned: that check, not the forms above,
//! is what refuses a form the edit cannot handle (a key inside an inline
//! table, one beside dotted keys in an inline table), rather than risk it.

use super::document::{Document, Entry, Form};
use crate::{
    layer::{Layer, LayerError, SCHEMA},
    registry::{Registry, SCHEMA_KEY},
    value::Value,
};
use std::{error, fmt, ops::Range};
use toml::de::DeTable;

/// `text`, a preferences file or `None` for a new one, with `key` set to
/// `value`.
///
/// # Errors
///
/// [`EditError::Invalid`] when `text` is not a valid preferences file, and
/// [`EditError::Unsafe`] when the change cannot be made safely.
pub fn set_in_document(
    registry: &Registry,
    text: Option<&str>,
    key: &str,
    value: &Value,
) -> Result<String, EditError> {
    let text = text.map_or_else(|| format!("{SCHEMA_KEY} = {SCHEMA:?}\n"), str::to_owned);
    let before = Layer::parse(registry, &text).map_err(EditError::Invalid)?;
    let unsafe_edit = || EditError::Unsafe(key.to_owned());
    let root = DeTable::parse(&text).map_err(|_| unsafe_edit())?;
    let document = Document::of(root.get_ref(), &text);
    let edited = match document.entry(key) {
        Some(entry) => splice(&text, entry.value.clone(), &value.to_toml()),
        None => add(&text, &document, key, value).ok_or_else(unsafe_edit)?,
    };
    verify(registry, &edited, &before.with(key, Some(value)), key)?;
    Ok(edited)
}

/// `text`, a preferences file, without `key`; `None` when it does not set
/// `key`.
///
/// # Errors
///
/// [`EditError::Invalid`] when `text` is not a valid preferences file, and
/// [`EditError::Unsafe`] when the key cannot be removed safely.
pub fn unset_in_document(
    registry: &Registry,
    text: &str,
    key: &str,
) -> Result<Option<String>, EditError> {
    let before = Layer::parse(registry, text).map_err(EditError::Invalid)?;
    if before.get(key).is_none() {
        return Ok(None);
    }
    let unsafe_edit = || EditError::Unsafe(key.to_owned());
    let root = DeTable::parse(text).map_err(|_| unsafe_edit())?;
    let document = Document::of(root.get_ref(), text);
    let entry = document.entry(key).ok_or_else(unsafe_edit)?;
    let lines = line_start(text, entry.key.start)..line_end(text, entry.value.end);
    let edited = splice(text, lines, "");
    verify(registry, &edited, &before.with(key, None), key)?;
    Ok(Some(edited))
}

/// Refuses `edited` unless it parses as exactly `expected`.
fn verify(registry: &Registry, edited: &str, expected: &Layer, key: &str) -> Result<(), EditError> {
    match Layer::parse(registry, edited) {
        Ok(layer) if layer == *expected => Ok(()),
        _ => Err(EditError::Unsafe(key.to_owned())),
    }
}

/// `text` with the bytes of `span` replaced by `replacement`.
fn splice(text: &str, span: Range<usize>, replacement: &str) -> String {
    let mut edited = String::with_capacity(text.len() + replacement.len());
    edited.push_str(text.get(..span.start).unwrap_or_default());
    edited.push_str(replacement);
    edited.push_str(text.get(span.end..).unwrap_or_default());
    edited
}

/// `text` with the line of `key` = `value` added in its table, `None` when
/// the table's form leaves no safe place for it.
fn add(text: &str, document: &Document, key: &str, value: &Value) -> Option<String> {
    let segments: Vec<&str> = key.split('.').collect();
    let table = (1..segments.len())
        .rev()
        .find_map(|length| document.table(&segments.get(..length)?.join(".")));
    let Some(table) = table else {
        let (last, parents) = segments.split_last()?;
        if parents.is_empty() {
            let after = last_line_end(text, document, 0, document.first_header(text));
            return Some(insert_line(
                text,
                after,
                &format!("{key} = {}", value.to_toml()),
            ));
        }
        return Some(append_table(text, &parents.join("."), last, value));
    };
    let rest = key.get(table.path.len() + 1..)?;
    let line = format!("{rest} = {}", value.to_toml());
    match table.form {
        Form::Header => {
            let start = line_end(text, table.span.end);
            let after = last_line_end(text, document, start, document.next_header(start, text));
            Some(insert_line(text, after.max(start), &line))
        }
        Form::Dotted => {
            let sibling = last_child(document, &table.path)?;
            let prefix = text.get(line_start(text, sibling.key.start)..sibling.key.start)?;
            let after = line_end(text, sibling.value.end);
            Some(insert_line(text, after, &format!("{prefix}{line}")))
        }
        Form::Inline => None,
    }
}

/// The key of `table`'s last line that is its direct child, by position.
fn last_child<'d>(document: &'d Document, table: &str) -> Option<&'d Entry> {
    document
        .entries
        .iter()
        .filter(|entry| {
            entry
                .path
                .strip_prefix(table)
                .and_then(|rest| rest.strip_prefix('.'))
                .is_some_and(|rest| !rest.contains('.'))
        })
        .max_by_key(|entry| entry.value.end)
}

/// Where the line after the last entry between `start` and `end` begins,
/// `start` when there is none.
fn last_line_end(text: &str, document: &Document, start: usize, end: usize) -> usize {
    document
        .entries
        .iter()
        .filter(|entry| (start..end).contains(&entry.key.start))
        .map(|entry| line_end(text, entry.value.end))
        .max()
        .unwrap_or(start)
}

/// `text` with `line` inserted at `offset`, a line's start, in the file's
/// line ending; at the end of a file without a last newline, the line
/// becomes the last one, without one either.
fn insert_line(text: &str, offset: usize, line: &str) -> String {
    let ending = line_ending(text);
    if offset >= text.len() && !text.is_empty() && !text.ends_with('\n') {
        return format!("{text}{ending}{line}");
    }
    splice(text, offset..offset, &format!("{line}{ending}"))
}

/// `text` with a new `[table]` holding `key` = `value` at its end.
fn append_table(text: &str, table: &str, key: &str, value: &Value) -> String {
    let ending = line_ending(text);
    let separator = if text.is_empty() || text.ends_with('\n') {
        ""
    } else {
        ending
    };
    format!(
        "{text}{separator}{ending}[{table}]{ending}{key} = {}{ending}",
        value.to_toml()
    )
}

/// The file's line ending: CRLF when it holds one, else LF.
fn line_ending(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// Where the line holding `offset` starts.
fn line_start(text: &str, offset: usize) -> usize {
    text.get(..offset)
        .and_then(|before| before.rfind('\n'))
        .map_or(0, |newline| newline + 1)
}

/// Where the line after the one holding `offset` starts, or the end of
/// `text`.
fn line_end(text: &str, offset: usize) -> usize {
    text.get(offset..)
        .and_then(|after| after.find('\n'))
        .map_or(text.len(), |newline| offset + newline + 1)
}

/// Why an edit was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// The file is not a valid preferences file.
    Invalid(LayerError),
    /// The key cannot be edited safely in the file's form.
    Unsafe(String),
}

impl fmt::Display for EditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => write!(formatter, "{error}"),
            Self::Unsafe(key) => write!(
                formatter,
                "{key} cannot be edited safely in this file's layout (an inline table or an \
                 unusual form); edit the file by hand"
            ),
        }
    }
}

impl error::Error for EditError {}
