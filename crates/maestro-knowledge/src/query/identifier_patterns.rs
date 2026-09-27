//! Parameter, command and path identifier patterns.

use super::identifier_types::{Candidate, Family};
use crate::lexical::is_stopword;

/// Collects assignment, flag and shell-variable spans for later arbitration.
pub(super) fn parameter_candidates(text: &str) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for (start, character) in text.char_indices() {
        let end = match character {
            '$' => variable_end(text, start),
            '-' if !has_word_before(text, start) => flag_end(text, start),
            character if is_assignment_start(character) && !has_word_before(text, start) => {
                assignment_end(text, start)
            }
            _ => None,
        };
        if let Some(end) = end {
            candidates.push(Candidate {
                family: Family::Parameter,
                start,
                end,
            });
        }
    }
    candidates
}

/// Returns the end of `$NAME` or `${NAME}` when its name is valid.
fn variable_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut name_start = start.checked_add(1)?;
    let braced = bytes.get(name_start) == Some(&b'{');
    if braced {
        name_start = name_start.checked_add(1)?;
    }
    if !bytes
        .get(name_start)
        .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
    {
        return None;
    }
    let mut end = name_start.checked_add(1)?;
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        end = end.checked_add(1)?;
    }
    if braced {
        if bytes.get(end) != Some(&b'}') {
            return None;
        }
        end.checked_add(1)
    } else {
        Some(end)
    }
}

/// Returns the end of a long flag or a one-letter short flag.
fn flag_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let first = start.checked_add(1)?;
    if bytes.get(first) == Some(&b'-') {
        let name_start = first.checked_add(1)?;
        if !bytes.get(name_start).is_some_and(u8::is_ascii_alphabetic) {
            return None;
        }
        let mut end = name_start.checked_add(1)?;
        while bytes
            .get(end)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
        {
            end = end.checked_add(1)?;
        }
        (!has_word_after(text, end)).then_some(end)
    } else if bytes.get(first).is_some_and(u8::is_ascii_alphabetic) {
        let end = first.checked_add(1)?;
        (!has_word_after(text, end)).then_some(end)
    } else {
        None
    }
}

/// Returns a nonempty assignment span without trailing sentence punctuation.
fn assignment_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut end = start;
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        end = end.checked_add(1)?;
    }
    if bytes.get(end) != Some(&b'=') {
        return None;
    }
    let value_start = end.checked_add(1)?;
    end = value_start;
    while bytes
        .get(end)
        .is_some_and(|byte| !byte.is_ascii_whitespace())
    {
        end = end.checked_add(1)?;
    }
    let value = text.get(value_start..end)?;
    let end = value_start.checked_add(value.trim_end_matches(is_trailing_punctuation).len())?;
    (end > value_start).then_some(end)
}

/// Whether a character may start an ASCII assignment name.
fn is_assignment_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

/// Whether punctuation is trimmed from the end of an identifier value.
fn is_trailing_punctuation(character: char) -> bool {
    matches!(character, ',' | '.' | ';' | '!' | '?' | ')' | ']' | '}')
}

/// Finds backtick contents and lowercase command spans containing parameters.
pub(super) fn command_candidates(text: &str, parameters: &[Candidate]) -> Vec<Candidate> {
    let mut candidates = backtick_candidates(text);
    for (start, character) in text.char_indices() {
        if !character.is_ascii_lowercase() || has_word_before(text, start) {
            continue;
        }
        let Some(word_end) = command_word_end(text, start) else {
            continue;
        };
        if is_stopword(&text[start..word_end]) {
            continue;
        }
        for parameter in parameters
            .iter()
            .filter(|parameter| parameter.start >= word_end)
        {
            let Some(gap) = text.get(word_end..parameter.start) else {
                continue;
            };
            if !is_command_gap(gap) {
                continue;
            }
            let end = extend_command_end(text, parameters, parameter.end);
            candidates.push(Candidate {
                family: Family::Command,
                start,
                end,
            });
            break;
        }
    }
    candidates
}

/// Returns the end of an ASCII lowercase command word.
fn command_word_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut end = start;
    while bytes.get(end).is_some_and(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_' || *byte == b'-'
    }) {
        end = end.checked_add(1)?;
    }
    Some(end)
}

/// Accepts only normalized spaces and lowercase subcommand words in a gap.
fn is_command_gap(gap: &str) -> bool {
    gap.starts_with(' ')
        && gap.ends_with(' ')
        && gap.split_whitespace().all(|word| {
            !is_stopword(word)
                && word.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && word.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || byte == b'_'
                        || byte == b'-'
                })
        })
}

/// Includes adjacent parameter tokens in a command's source span.
fn extend_command_end(text: &str, parameters: &[Candidate], first_end: usize) -> usize {
    let mut end = first_end;
    for parameter in parameters {
        let Some(gap) = text.get(end..parameter.start) else {
            continue;
        };
        if gap.is_empty() || !gap.chars().all(char::is_whitespace) {
            break;
        }
        end = parameter.end;
    }
    end
}

/// Extracts the exact contents of paired backtick delimiters.
fn backtick_candidates(text: &str) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = text.get(cursor..).and_then(|tail| tail.find('`')) {
        let Some(start) = cursor.checked_add(relative_start) else {
            break;
        };
        let Some((delimiter_length, content_start)) = backtick_run(text, start) else {
            break;
        };
        let mut search = content_start;
        let close = loop {
            let Some(relative_close) = text.get(search..).and_then(|tail| tail.find('`')) else {
                break None;
            };
            let Some(close_start) = search.checked_add(relative_close) else {
                break None;
            };
            let Some((close_length, close_end)) = backtick_run(text, close_start) else {
                break None;
            };
            if close_length == delimiter_length {
                break Some((close_start, close_end));
            }
            search = close_end;
        };
        let Some((close_start, close_end)) = close else {
            break;
        };
        candidates.push(Candidate {
            family: Family::Command,
            start: content_start,
            end: close_start,
        });
        cursor = close_end;
    }
    candidates
}

/// Returns the number of consecutive backticks and the byte after them.
fn backtick_run(text: &str, start: usize) -> Option<(usize, usize)> {
    let run_length = text
        .get(start..)?
        .chars()
        .take_while(|character| matches!(*character, '`'))
        .count();
    let end = start.checked_add(run_length)?;
    Some((run_length, end))
}

/// Finds supported Unix, Windows and filename path spans.
pub(super) fn path_candidates(text: &str) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for (start, character) in text.char_indices() {
        if has_word_before(text, start) {
            continue;
        }
        let end = if character.is_ascii_alphabetic() {
            windows_path_end(text, start)
        } else {
            None
        }
        .or_else(|| unix_path_end(text, start))
        .or_else(|| filename_end(text, start));
        if let Some(end) = end {
            candidates.push(Candidate {
                family: Family::Path,
                start,
                end,
            });
        }
    }
    candidates
}

/// Returns a drive-letter path's end when it contains a path segment.
fn windows_path_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let separator = start.checked_add(1)?;
    let path_start = separator.checked_add(2)?;
    if !bytes.get(start).is_some_and(u8::is_ascii_alphabetic)
        || bytes.get(separator) != Some(&b':')
        || bytes.get(separator.checked_add(1)?) != Some(&b'\\')
    {
        return None;
    }
    let mut end = path_start;
    let mut has_segment = false;
    while bytes.get(end).is_some_and(|byte| is_path_byte(*byte)) {
        has_segment |= !bytes
            .get(end)
            .is_some_and(|byte| matches!(byte, b'\\' | b'/'));
        end = end.checked_add(1)?;
    }
    if !has_segment {
        return None;
    }
    Some(trim_path_end(text, start, end))
}

/// Returns the end of a rooted or explicitly relative Unix path.
fn unix_path_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let prefix_length = if bytes.get(start) == Some(&b'/') {
        if has_character_before(text, start, ':') || has_character_before(text, start, '/') {
            return None;
        }
        1
    } else if bytes.get(start..start.checked_add(2)?) == Some(b"./")
        || bytes.get(start..start.checked_add(2)?) == Some(b"~/")
    {
        2
    } else if bytes.get(start..start.checked_add(3)?) == Some(b"../") {
        3
    } else {
        return None;
    };
    let path_start = start.checked_add(prefix_length)?;
    let mut end = path_start;
    let mut has_segment = false;
    while bytes.get(end).is_some_and(|byte| is_path_byte(*byte)) {
        has_segment |= bytes.get(end).is_none_or(|byte| *byte != b'/');
        end = end.checked_add(1)?;
    }
    if !has_segment {
        return None;
    }
    Some(trim_path_end(text, start, end))
}

/// Whether a byte can occur inside the supported ASCII path forms.
fn is_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'~' | b'/' | b'\\')
}

/// Removes sentence punctuation from a path's captured end.
fn trim_path_end(text: &str, start: usize, end: usize) -> usize {
    text.get(start..end).map_or(end, |path| {
        start + path.trim_end_matches(is_trailing_punctuation).len()
    })
}

/// Returns a filename end only when its extension begins with a letter.
fn filename_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if !bytes.get(start).is_some_and(|byte| is_filename_byte(*byte)) {
        return None;
    }
    let mut end = start;
    while bytes.get(end).is_some_and(|byte| is_filename_byte(*byte)) {
        end = end.checked_add(1)?;
    }
    let end = trim_path_end(text, start, end);
    let filename = text.get(start..end)?;
    // Extensions start with a letter, so a dotted version such as `1.2` is not a filename.
    let dot = filename.rfind('.')?;
    let extension = filename.get(dot.checked_add(1)?..)?;
    if dot == 0
        || !extension
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
    {
        return None;
    }
    Some(end)
}

/// Whether a byte belongs to the supported filename token alphabet.
fn is_filename_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
}

/// Whether the previous Unicode character belongs to a word.
pub(super) fn has_word_before(text: &str, position: usize) -> bool {
    text.get(..position)
        .and_then(|prefix| prefix.chars().next_back())
        .is_some_and(is_word_character)
}

/// Whether the next Unicode character belongs to a word.
pub(super) fn has_word_after(text: &str, position: usize) -> bool {
    text.get(position..)
        .and_then(|suffix| suffix.chars().next())
        .is_some_and(is_word_character)
}

/// Whether a character is alphanumeric or an underscore.
pub(super) fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// Whether the character immediately before a span is the requested one.
fn has_character_before(text: &str, position: usize, expected: char) -> bool {
    text.get(..position)
        .and_then(|prefix| prefix.chars().next_back())
        == Some(expected)
}
