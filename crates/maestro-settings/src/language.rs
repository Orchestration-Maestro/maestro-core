//! The language tags `language` accepts: the bounded BCP 47 subset S3 ruled
//! (plan D6), parsed without a registry lookup or a library. A tag is a 2-3
//! letter language, then optionally a 4-letter script, then optionally a
//! 2-letter or 3-digit region; each is stored in canonical case (`en`,
//! `zh-Hant-TW`, `es-419`). Variants, extensions, private use, irregular legacy
//! forms and any other subtag are refused by name. It is not full BCP 47
//! support.

use std::ops::RangeInclusive;

/// The canonical form of the language tag `text`.
///
/// # Errors
///
/// The reason `text` is not a tag of the subset.
pub fn canonical_language(text: &str) -> Result<String, &'static str> {
    if !text.is_ascii() {
        return Err("a language tag is ASCII");
    }
    let mut subtags = text.split('-');
    let language = subtags.next().unwrap_or_default();
    if !is_alphabetic(language, 2..=3) {
        return Err(if language.len() == 1 {
            "private-use and irregular legacy tags are not supported"
        } else {
            "a language tag starts with a 2 or 3 letter language"
        });
    }
    let mut canonical = language.to_ascii_lowercase();
    let mut next = subtags.next();
    if let Some(script) = next.filter(|script| is_alphabetic(script, 4..=4)) {
        canonical.push('-');
        canonical.push_str(&title_case(script));
        next = subtags.next();
    }
    if let Some(region) = next.filter(|region| is_region(region)) {
        canonical.push('-');
        canonical.push_str(&region.to_ascii_uppercase());
        next = subtags.next();
    }
    match next {
        None => Ok(canonical),
        Some("") => Err("a language tag has an empty subtag"),
        Some(_) => Err("only a language, a script and a region are supported: \
             variants, extensions and private use are not"),
    }
}

/// Whether `subtag` is ASCII letters, as many as `lengths` allows.
fn is_alphabetic(subtag: &str, lengths: RangeInclusive<usize>) -> bool {
    lengths.contains(&subtag.len()) && subtag.bytes().all(|byte| byte.is_ascii_alphabetic())
}

/// Whether `subtag` is a region: two letters or three digits.
fn is_region(subtag: &str) -> bool {
    is_alphabetic(subtag, 2..=2)
        || (subtag.len() == 3 && subtag.bytes().all(|byte| byte.is_ascii_digit()))
}

/// `script` in title case, as BCP 47 writes a script: `Hant`.
fn title_case(script: &str) -> String {
    let lower = script.to_ascii_lowercase();
    let mut characters = lower.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_ascii_uppercase().to_string() + characters.as_str()
    })
}
