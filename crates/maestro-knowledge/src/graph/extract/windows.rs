//! Versioned, source-byte-bounded windows and quote pointers.

use super::super::verify::Source;
use maestro_canonicalization::SourceSpan;
use serde::Deserialize;
use std::{collections::BTreeSet, error, fmt};

/// A versioned windowing policy. The digest of its source file is frozen by the build.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowPolicy {
    /// The policy contract.
    pub schema: String,
    /// Maximum original-source bytes in one call window.
    pub max_window_bytes: usize,
    /// Bytes shared by adjacent windows of an oversized block.
    pub overlap_bytes: usize,
    /// Maximum model calls for one source revision.
    pub max_windows: usize,
}

/// Why a window policy cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowPolicyError(&'static str);

/// One exact source window, owned by one canonical block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// Canonical source block that owns this window.
    pub block_id: String,
    /// Original Markdown byte range.
    pub span: SourceSpan,
    /// Exact UTF-8 source bytes in `span`.
    pub text: String,
}

impl WindowPolicy {
    /// Parses and validates a closed `maestro-graph-window-policy/1` file.
    ///
    /// # Errors
    ///
    /// Returns a fixed reason without echoing policy contents.
    pub fn parse(text: &str) -> Result<Self, WindowPolicyError> {
        let policy: Self =
            serde_json::from_str(text).map_err(|_| WindowPolicyError("invalid window policy"))?;
        if policy.schema != "maestro-graph-window-policy/1"
            || policy.max_window_bytes == 0
            || policy.max_windows == 0
            || policy.overlap_bytes >= policy.max_window_bytes
        {
            return Err(WindowPolicyError("invalid window policy bounds"));
        }
        Ok(policy)
    }
}

/// Builds bounded windows from leaf canonical blocks and unchanged Markdown.
///
/// # Errors
///
/// Refuses malformed block spans, oversized window counts or invalid UTF-8 boundaries.
pub fn windows(source: &Source, policy: &WindowPolicy) -> Result<Vec<Window>, &'static str> {
    let mut parents = BTreeSet::new();
    for block in &source.canonical().blocks {
        if let Some(parent) = &block.parent_block_id {
            parents.insert(parent.as_str());
        }
    }
    let markdown = source.markdown();
    let mut result = Vec::new();
    for block in &source.canonical().blocks {
        if block.revision_id != source.revision_id() {
            return Err("block belongs to another revision");
        }
        if parents.contains(block.block_id.as_str()) {
            continue;
        }
        let [span] = block.source_spans.as_slice() else {
            return Err("ambiguous source block span");
        };
        if !span.is_valid(markdown) || span.start == span.end {
            return Err("invalid source block span");
        }
        let mut start = span.start;
        while start < span.end {
            if result.len() >= policy.max_windows {
                return Err("window count exceeds policy");
            }
            let mut end = start.saturating_add(policy.max_window_bytes).min(span.end);
            while end > start && !markdown.is_char_boundary(end) {
                end -= 1;
            }
            if end == start {
                return Err("window policy splits a UTF-8 character");
            }
            let text = markdown
                .get(start..end)
                .ok_or("invalid source block span")?;
            result.push(Window {
                block_id: block.block_id.clone(),
                span: SourceSpan { start, end },
                text: text.to_owned(),
            });
            if end == span.end {
                break;
            }
            let mut next = end.saturating_sub(policy.overlap_bytes);
            while next > start && !markdown.is_char_boundary(next) {
                next -= 1;
            }
            if next <= start {
                return Err("window overlap prevents progress");
            }
            start = next;
        }
    }
    Ok(result)
}

/// Locates a model-supplied quote exactly once in a window and returns its source bytes.
///
/// # Errors
///
/// Returns a fixed refusal when the quote is empty, absent or ambiguous.
pub fn locate_quote(window: &Window, quote: &str) -> Result<SourceSpan, &'static str> {
    if quote.is_empty() {
        return Err("empty quote");
    }
    let mut found = None;
    let mut from = 0;
    while let Some(relative) = window.text[from..].find(quote) {
        let start = from + relative;
        let end = start + quote.len();
        if !window.text.is_char_boundary(start) || !window.text.is_char_boundary(end) {
            return Err("quote splits a UTF-8 character");
        }
        if found.is_some() {
            return Err("ambiguous quote");
        }
        found = Some(SourceSpan {
            start: window.span.start + start,
            end: window.span.start + end,
        });
        from = start + 1;
        while from < window.text.len() && !window.text.is_char_boundary(from) {
            from += 1;
        }
        if from >= window.text.len() {
            break;
        }
    }
    found.ok_or("quote is not in its source window")
}

impl fmt::Display for WindowPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl error::Error for WindowPolicyError {}
