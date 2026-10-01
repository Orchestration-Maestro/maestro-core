//! Version, port and error-code identifier patterns.

use super::identifier_patterns::{has_word_after, has_word_before};
use super::identifier_types::{Candidate, Family};

/// Finds version spans while rejecting suffixes of longer dotted numbers.
pub(super) fn version_candidates(text: &str) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for (start, character) in text.char_indices() {
        if !character.is_ascii_digit() && character != 'v' {
            continue;
        }
        if has_word_before(text, start) || has_numeric_component_before(text, start) {
            continue;
        }
        if let Some(end) = version_end(text, start, character) {
            candidates.push(Candidate {
                family: Family::Version,
                start,
                end,
            });
        }
    }
    candidates
}

/// Whether a digit begins after an earlier numeric dotted component.
fn has_numeric_component_before(text: &str, position: usize) -> bool {
    let Some(prefix) = text.get(..position) else {
        return false;
    };
    let mut characters = prefix.chars().rev();
    characters.next() == Some('.')
        && characters
            .next()
            .is_some_and(|value| value.is_ascii_digit())
}

/// Returns a complete version span with one to four numeric components.
fn version_end(text: &str, start: usize, character: char) -> Option<usize> {
    let bytes = text.as_bytes();
    let has_prefix = character == 'v';
    let mut end = if has_prefix {
        start.checked_add(1)?
    } else {
        start
    };
    let first_digit = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end = end.checked_add(1)?;
    }
    if end == first_digit {
        return None;
    }
    let mut components = 1;
    while bytes.get(end) == Some(&b'.')
        && bytes
            .get(end.checked_add(1)?)
            .is_some_and(u8::is_ascii_digit)
    {
        components += 1;
        if components > 4 {
            return None;
        }
        end = end.checked_add(1)?;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end = end.checked_add(1)?;
        }
    }
    if components == 1 && !has_prefix {
        return None;
    }
    if has_word_after(text, end) {
        return None;
    }
    Some(end)
}

/// Finds in-range decimal ports after `port ` and colon prefixes.
pub(super) fn port_candidates(text: &str) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for (start, character) in text.char_indices() {
        let number_start = if character == ':' {
            start.checked_add(1)
        } else if matches!(character, 'p' | 'P') && !has_word_before(text, start) {
            port_keyword_number_start(text, start)
        } else {
            None
        };
        if let Some(number_start) = number_start
            && let Some(end) = valid_port_end(text, number_start)
        {
            candidates.push(Candidate {
                family: Family::Port,
                start: number_start,
                end,
            });
        }
    }
    candidates
}

/// Returns the byte after a case-insensitive `port` keyword and whitespace.
fn port_keyword_number_start(text: &str, start: usize) -> Option<usize> {
    let keyword_end = start.checked_add(4)?;
    if !text
        .get(start..keyword_end)
        .is_some_and(|word| word.eq_ignore_ascii_case("port"))
    {
        return None;
    }
    let whitespace_end = text
        .get(keyword_end..)?
        .char_indices()
        .next()
        .filter(|(_, character)| character.is_whitespace())
        .and_then(|(offset, character)| offset.checked_add(character.len_utf8()))?;
    keyword_end.checked_add(whitespace_end)
}

/// Returns a complete port number only when its value is within range.
fn valid_port_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if !bytes.get(start).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    let mut end = start;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end = end.checked_add(1)?;
    }
    if has_word_after(text, end) {
        return None;
    }
    let following = text.get(end..).and_then(|tail| tail.chars().next());
    if following == Some('.')
        && text
            .get(end.checked_add(1)?..)
            .and_then(|tail| tail.chars().next())
            .is_some_and(|character| character.is_ascii_digit())
    {
        return None;
    }
    let value = text.get(start..end)?.parse::<u32>().ok()?;
    (1..=65_535).contains(&value).then_some(end)
}

/// Finds uppercase error-code spans at Unicode word boundaries.
pub(super) fn error_code_candidates(text: &str) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for (start, character) in text.char_indices() {
        if !character.is_ascii_uppercase() || has_word_before(text, start) {
            continue;
        }
        if let Some(end) = error_code_end(text, start) {
            candidates.push(Candidate {
                family: Family::ErrorCode,
                start,
                end,
            });
        }
    }
    candidates
}

/// Returns an entire error code only when all component lengths are valid.
fn error_code_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut end = start;
    while bytes.get(end).is_some_and(u8::is_ascii_uppercase) {
        end = end.checked_add(1)?;
    }
    let capital_count = end.checked_sub(start)?;
    if !(1..=6).contains(&capital_count) {
        return None;
    }
    if matches!(bytes.get(end), Some(b'-' | b'_')) {
        end = end.checked_add(1)?;
    }
    let digits_start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end = end.checked_add(1)?;
    }
    let digit_count = end.checked_sub(digits_start)?;
    if !(2..=6).contains(&digit_count) {
        return None;
    }
    if bytes.get(end).is_some_and(u8::is_ascii_uppercase) {
        end = end.checked_add(1)?;
    }
    (!has_word_after(text, end)).then_some(end)
}
