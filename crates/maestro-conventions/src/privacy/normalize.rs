//! Normalizes private text and fingerprints complete token windows.

use super::mac::{HmacSha256, hmac_sha256};
use std::{io, mem};
use unicode_normalization::{
    UnicodeNormalization,
    char::{canonical_combining_class, decompose_compatible},
};

/// Number of normalized tokens in each overlap-tested shingle.
pub(super) const SHINGLE_SIZE: usize = 8;
/// Shingle length stored in the bank's integer schema.
pub(super) const SHINGLE_LENGTH: u64 = 8;
/// Bank kind tag for complete private units of four to seven tokens.
pub(super) const SHORT_UNIT_TAG: u8 = 3;

/// A normalized token and its half-open byte range in the source text.
#[derive(Clone, Debug)]
pub(super) struct Token {
    /// NFKC-lowercased token value used for matching.
    pub(super) text: String,
    /// Inclusive source byte offset.
    pub(super) start: usize,
    /// Exclusive source byte offset.
    pub(super) end: usize,
}

/// Splits text into words after whole-segment NFKC normalization.
pub(super) fn tokens(text: &str) -> Vec<Token> {
    let mut tokenizer = Tokenizer::default();
    let mut segment = String::new();
    let mut segment_start = 0;
    let mut segment_end = 0;
    for (offset, character) in text.char_indices() {
        let after = offset + character.len_utf8();
        if starts_normalization_segment(character) && !segment.is_empty() {
            tokenizer.push_segment(&segment, segment_start, segment_end);
            segment.clear();
        }
        if segment.is_empty() {
            segment_start = offset;
        }
        segment.push(character);
        segment_end = after;
    }
    tokenizer.push_segment(&segment, segment_start, segment_end);
    tokenizer.finish()
}

/// Accumulates normalized words while retaining their source byte ranges.
#[derive(Debug, Default)]
struct Tokenizer {
    /// Completed tokens.
    words: Vec<Token>,
    /// Current token under construction.
    word: String,
    /// Start byte offset of the current token.
    start: usize,
    /// End byte offset of the current token.
    end: usize,
}

impl Tokenizer {
    /// Normalizes one combining sequence and updates the current token.
    fn push_segment(&mut self, segment: &str, source_start: usize, source_end: usize) {
        for normalized in segment.nfkc().flat_map(char::to_lowercase) {
            let is_word_character = normalized.is_alphanumeric();
            if is_word_character && self.word.is_empty() {
                self.start = source_start;
            }
            if is_word_character {
                self.word.push(normalized);
                self.end = source_end;
            } else {
                self.push_word();
            }
        }
    }

    /// Finishes the current token, if any.
    fn push_word(&mut self) {
        if !self.word.is_empty() {
            self.words.push(Token {
                text: mem::take(&mut self.word),
                start: self.start,
                end: self.end,
            });
        }
    }

    /// Flushes the final token and returns all tokens.
    fn finish(mut self) -> Vec<Token> {
        self.push_word();
        self.words
    }
}

/// Checks whether a character begins a new compatibility-normalization segment.
fn starts_normalization_segment(character: char) -> bool {
    let mut first_class = None;
    decompose_compatible(character, |decomposed| {
        if first_class.is_none() {
            first_class = Some(canonical_combining_class(decomposed));
        }
    });
    first_class.is_some_and(|class| class == 0)
}

/// Computes a domain-separated keyed fingerprint over one byte string.
pub(super) fn tag(key: &[u8; 32], kind: u8, length: u64, content: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new(key);
    mac.update(b"maestro-privacy-tag-v1\0");
    mac.update(&[kind]);
    mac.update(&length.to_be_bytes());
    mac.update(
        &u64::try_from(content.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    mac.update(content);
    mac.finalize()
}

/// Derives the non-secret identifier used to bind a bank to its key.
pub(super) fn key_id(key: &[u8; 32]) -> String {
    hex(&hmac_sha256(key, b"maestro-privacy-key-id-v1\0"))
}

/// Validates JSON streams and decodes every string literal, including keys.
pub(super) fn json_strings(bytes: &[u8]) -> Result<Vec<String>, io::Error> {
    let Some(first) = bytes
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
    else {
        return Ok(Vec::new());
    };
    if !matches!(first, b'{' | b'[' | b'\"') {
        return Ok(Vec::new());
    }
    for value in serde_json::Deserializer::from_slice(bytes).into_iter::<serde_json::Value>() {
        value.map_err(|_| io::Error::other("invalid encoded JSON"))?;
    }
    let mut strings = Vec::new();
    let mut start = None;
    let mut escaped = false;
    for (index, byte) in bytes.iter().enumerate() {
        if start.is_none() {
            if *byte == b'\"' {
                start = Some(index);
            }
        } else if escaped {
            escaped = false;
        } else if *byte == b'\\' {
            escaped = true;
        } else if *byte == b'\"' {
            let begin = start
                .take()
                .ok_or_else(|| io::Error::other("invalid JSON string"))?;
            let encoded = bytes
                .get(begin..=index)
                .ok_or_else(|| io::Error::other("invalid JSON string"))?;
            strings.push(
                serde_json::from_slice(encoded)
                    .map_err(|_| io::Error::other("invalid JSON string"))?,
            );
        }
    }
    if start.is_some() || escaped {
        return Err(io::Error::other("invalid JSON string"));
    }
    Ok(strings)
}

/// Encodes bytes as lowercase hexadecimal.
pub(super) fn hex(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        result.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    result
}

/// Fingerprints one normalized token shingle.
pub(super) fn shingle_tag(key: &[u8; 32], words: &[Token]) -> [u8; 32] {
    tag(key, 2, SHINGLE_LENGTH, &normalized_content(words))
}

/// Fingerprints one complete private question or report unit.
pub(super) fn short_unit_tag(key: &[u8; 32], words: &[Token]) -> [u8; 32] {
    let length = u64::try_from(words.len()).unwrap_or(u64::MAX);
    tag(key, SHORT_UNIT_TAG, length, &normalized_content(words))
}

/// Joins normalized words with one ASCII space for stable matching.
fn normalized_content(words: &[Token]) -> Vec<u8> {
    let mut content = Vec::new();
    for (index, token) in words.iter().enumerate() {
        if index > 0 {
            content.push(b' ');
        }
        content.extend_from_slice(token.text.as_bytes());
    }
    content
}

/// Unicode and shingle normalization tests.
#[cfg(test)]
mod tests {
    use super::{SHINGLE_SIZE, tokens};

    #[test]
    /// Applies compatibility normalization, lowercase and token boundaries.
    fn normalization_applies_nfkc_lowercase_and_word_boundaries() {
        let actual = tokens("ＱＵＡＲＴＺ—Zephyr\r\nlantern");
        assert_eq!(
            actual
                .iter()
                .map(|token| token.text.as_str())
                .collect::<Vec<_>>(),
            ["quartz", "zephyr", "lantern"]
        );
    }

    #[test]
    /// Composes decomposed accents so equivalent source text matches.
    fn normalization_preserves_combining_sequences() {
        let actual = tokens("cafe\u{301}");
        assert_eq!(
            actual.first().map(|token| token.text.as_str()),
            Some("café")
        );
    }

    #[test]
    /// Requires exactly eight tokens before a shingle can be fingerprinted.
    fn only_full_overlapping_shingle_windows_can_be_fingerprinted() {
        assert_eq!(SHINGLE_SIZE, 8);
        assert_eq!(tokens("one two three four five six seven").len(), 7);
        assert_eq!(tokens("one two three four five six seven eight").len(), 8);
    }
}
