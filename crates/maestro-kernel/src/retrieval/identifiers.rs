//! Shared literal identifier matching and query-whitespace normalization.

/// Trims `text` and collapses each Unicode-whitespace run to one ASCII space,
/// preserving every other character exactly.
#[must_use]
pub fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether `identifier` occurs literally in `text` as a complete atom.
///
/// Text whitespace is normalized as it is for query understanding. Matching
/// remains case-, accent-, and punctuation-sensitive. A final period is a
/// delimiter only at the end of the text or immediately before a delimiter.
#[must_use]
pub fn contains_identifier(text: &str, identifier: &str) -> bool {
    if identifier.is_empty() {
        return false;
    }

    let text = normalize_whitespace(text);
    let mut remaining = text.as_str();
    let mut previous = None;
    while let Some(relative_start) = remaining.find(identifier) {
        let (before, candidate) = remaining.split_at(relative_start);
        if left_boundary(before.chars().next_back().or(previous))
            && let Some(after_identifier) = candidate.strip_prefix(identifier)
            && right_boundary(after_identifier)
        {
            return true;
        }
        let Some(character) = candidate.chars().next() else {
            return false;
        };
        previous = Some(character);
        remaining = candidate.split_at(character.len_utf8()).1;
    }
    false
}

/// Whether the character before a candidate, if any, is a permitted edge.
fn left_boundary(previous: Option<char>) -> bool {
    previous.is_none_or(delimiter)
}

/// Whether the character after an identifier, if any, is a permitted edge.
fn right_boundary(after_identifier: &str) -> bool {
    match after_identifier.chars().next() {
        None => true,
        Some('.') => after_identifier[1..].chars().next().is_none_or(delimiter),
        Some(character) => delimiter(character),
    }
}

/// The punctuation and whitespace that terminate an identifier atom.
fn delimiter(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '(' | ')'
                | '['
                | ']'
                | '<'
                | '>'
                | ','
                | ';'
                | ':'
                | '!'
                | '?'
                | '"'
                | '\''
                | '`'
                | '|'
        )
}
