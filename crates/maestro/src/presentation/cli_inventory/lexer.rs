//! Test-only Rust lexical scan: comments and character literals cannot invent messages.

/// A source token with its exact spelling and current source location.
#[derive(Debug)]
pub(super) struct Token<'a> {
    /// Literal contents, or an identifier/punctuation token.
    pub(super) text: &'a str,
    /// Whether this token is a string literal.
    pub(super) string: bool,
    /// Byte offset for reporting the current line, never an inventory identity.
    pub(super) offset: usize,
}

/// Scan Rust source, retaining raw/escaped/multiline string bytes without interpretation.
pub(super) fn tokens(source: &str) -> Vec<Token<'_>> {
    let bytes = source.as_bytes();
    let mut cursor = 0;
    let mut result = Vec::new();
    while cursor < bytes.len() {
        let start = cursor;
        if source[cursor..].starts_with("//") {
            cursor += source[cursor..].find('\n').unwrap_or(bytes.len() - cursor);
        } else if source[cursor..].starts_with("/*") {
            cursor = comment_end(source, cursor);
        } else if let Some((content, end)) = raw_string(source, cursor) {
            result.push(Token {
                text: &source[content..end.0],
                string: true,
                offset: start,
            });
            cursor = end.1;
        } else if bytes[cursor] == b'"' {
            cursor = quoted_end(bytes, cursor);
            result.push(Token {
                text: &source[start + 1..cursor - 1],
                string: true,
                offset: start,
            });
        } else if let Some(end) = character_end(source, cursor) {
            cursor = end;
        } else if bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'_' {
            cursor += 1;
            while cursor < bytes.len()
                && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'_')
            {
                cursor += 1;
            }
            result.push(Token {
                text: &source[start..cursor],
                string: false,
                offset: start,
            });
        } else {
            cursor += source[cursor..].chars().next().unwrap().len_utf8();
            if !source[start..cursor].trim().is_empty() {
                result.push(Token {
                    text: &source[start..cursor],
                    string: false,
                    offset: start,
                });
            }
        }
    }
    result
}

/// Skip nested block comments; malformed input fails rather than hanging.
fn comment_end(source: &str, start: usize) -> usize {
    let mut cursor = start + 2;
    let mut depth = 1;
    while cursor < source.len() {
        if source[cursor..].starts_with("/*") {
            depth += 1;
            cursor += 2;
        } else if source[cursor..].starts_with("*/") {
            depth -= 1;
            cursor += 2;
            if depth == 0 {
                return cursor;
            }
        } else {
            cursor += source[cursor..].chars().next().unwrap().len_utf8();
        }
    }
    panic!("unclosed Rust comment");
}

/// Raw, byte-raw and C-raw strings share the same quote/hash delimiter.
fn raw_string(source: &str, start: usize) -> Option<(usize, (usize, usize))> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    if matches!(bytes.get(cursor), Some(b'b' | b'c')) {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'r') {
        return None;
    }
    cursor += 1;
    let hashes = cursor;
    while bytes.get(cursor) == Some(&b'#') {
        cursor += 1;
    }
    let count = cursor - hashes;
    if bytes.get(cursor) != Some(&b'"') {
        return None;
    }
    let content = cursor + 1;
    cursor = content;
    while cursor < bytes.len() {
        if bytes[cursor] == b'"'
            && bytes
                .get(cursor + 1..cursor + 1 + count)
                .is_some_and(|tail| tail.iter().all(|byte| *byte == b'#'))
        {
            return Some((content, (cursor, cursor + count + 1)));
        }
        cursor += 1;
    }
    panic!("unclosed Rust raw string");
}

/// Escaped quotes and line continuations stay inside the same literal.
fn quoted_end(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'\\' {
            cursor += 2;
        } else if bytes[cursor] == b'"' {
            return cursor + 1;
        } else {
            cursor += 1;
        }
    }
    panic!("unclosed Rust string");
}

/// Recognize a single character, not a lifetime or label beginning with an apostrophe.
fn character_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.get(start) != Some(&b'\'') {
        return None;
    }
    let mut cursor = start + 1;
    if bytes.get(cursor) == Some(&b'\\') {
        cursor += 1;
        match bytes.get(cursor) {
            Some(b'u') => {
                cursor += source[cursor..].find('}')? + 1;
            }
            Some(b'x') => {
                cursor += 3;
            }
            Some(_) => {
                cursor += 1;
            }
            None => return None,
        }
    } else {
        cursor += source.get(cursor..)?.chars().next()?.len_utf8();
    }
    (bytes.get(cursor) == Some(&b'\'')).then_some(cursor + 1)
}

#[test]
fn catalog_presentation_inventory_lexer_preserves_real_strings_only() {
    let source = concat!(
        "// \"comment\"\n/* outer \"comment\" /* nested */ */ ",
        "r##\"raw \" quote\nline\"## br#\"bytes\"# cr\"C string\" ",
        "\"escaped \\\" quote\" \"line\\\ncontinued\" ",
        "'x' '\\u{e9}' '\\x41' '\\\'' '\"' 'static: label \"last\""
    );
    let strings: Vec<_> = tokens(source)
        .into_iter()
        .filter(|token| token.string)
        .map(|token| token.text)
        .collect();
    assert_eq!(
        strings,
        [
            "raw \" quote\nline",
            "bytes",
            "C string",
            "escaped \\\" quote",
            "line\\\ncontinued",
            "last"
        ]
    );
}
