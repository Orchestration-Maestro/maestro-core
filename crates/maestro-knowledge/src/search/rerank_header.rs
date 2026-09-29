//! Bounded, reranker-only heading prefixes.

use serde::{Deserialize, Serialize};
use std::iter;

/// Metadata prefixed to reranker documents, never to evidence text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RerankHeader {
    /// Preserve the indexed or enriched input exactly.
    #[default]
    Off,
    /// Page title followed by the section's heading path.
    HeadingPath,
}

impl RerankHeader {
    /// Whether serialization can omit this default setting.
    #[must_use]
    pub const fn is_off(&self) -> bool {
        matches!(self, Self::Off)
    }
}

/// Maximum UTF-8 bytes of the prefix, including separators and the blank line.
const HEADER_BYTES: usize = 512;

/// Normalizes and bounds the title and heading path, including its blank line.
/// On overflow the title keeps at most half the space, then the deepest
/// headings take the rest, in their original order.
pub(super) fn heading_path(title: Option<&str>, path: &[String]) -> Option<String> {
    let mut title = normalized(title.unwrap_or_default());
    let path = path
        .iter()
        .map(|part| normalized(part))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let content_budget = HEADER_BYTES - 2;
    let parts = iter::once(title.as_str())
        .filter(|part| !part.is_empty())
        .chain(path.iter().map(String::as_str))
        .collect::<Vec<_>>();
    let complete = parts.join(" > ");
    if complete.is_empty() {
        return None;
    }
    if complete.len() <= content_budget {
        return Some(format!("{complete}\n\n"));
    }
    truncate(
        &mut title,
        if path.is_empty() {
            content_budget
        } else {
            content_budget / 2
        },
    );
    let mut remaining = content_budget - title.len();
    let mut headings = Vec::new();
    for mut part in path.into_iter().rev() {
        let separator = if title.is_empty() && headings.is_empty() {
            0
        } else {
            3
        };
        let budget = remaining.saturating_sub(separator);
        if budget == 0 {
            break;
        }
        truncate(&mut part, budget);
        if part.is_empty() {
            break;
        }
        remaining -= separator + part.len();
        headings.push(part);
    }
    if !title.is_empty() {
        headings.push(title);
    }
    if headings.is_empty() {
        return None;
    }
    headings.reverse();
    Some(format!("{}\n\n", headings.join(" > ")))
}

/// Collapses all heading whitespace without rewriting punctuation or Unicode.
fn normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Cuts only at a character boundary, removing any trailing normalized space.
fn truncate(text: &mut String, limit: usize) {
    let end = text.floor_char_boundary(limit.min(text.len()));
    text.truncate(end);
    text.truncate(text.trim_end().len());
}
