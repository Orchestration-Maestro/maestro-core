//! Splitting: the identifiers and words of a folded text, and the parts of
//! its camelCase words.

/// A piece of a folded text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Piece<'text> {
    /// An identifier's whole form, a term once lowercased.
    Identifier(&'text str),
    /// A word, a term once lowercased, if it is not a stopword, and stemmed.
    Word(&'text str),
}

/// The pieces of `folded`, in order: an identifier's whole form before the
/// words it joins, and a camelCase word before its parts.
pub(super) fn pieces(folded: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let chunks = folded
        .split(|character: char| !(character.is_alphanumeric() || is_joiner(character)))
        .map(|chunk| chunk.trim_matches(is_joiner));
    for chunk in chunks {
        if chunk.contains(is_joiner) {
            pieces.push(Piece::Identifier(chunk));
        }
        for word in chunk.split(is_joiner).filter(|word| !word.is_empty()) {
            pieces.push(Piece::Word(word));
            let parts = camel_case_parts(word);
            if parts.len() > 1 {
                pieces.extend(parts.into_iter().map(Piece::Word));
            }
        }
    }
    pieces
}

/// Whether `character` joins the runs of letters and digits of an
/// identifier.
fn is_joiner(character: char) -> bool {
    matches!(character, '-' | '_' | '.' | '/' | ':')
}

/// The camelCase parts of `word`: the word itself when it has none.
fn camel_case_parts(word: &str) -> Vec<&str> {
    let letters: Vec<Option<char>> = [None, None]
        .into_iter()
        .chain(word.chars().map(Some))
        .chain([None, None])
        .collect();
    let starts =
        word.char_indices()
            .zip(letters.windows(5))
            .filter_map(|((start, current), around)| match *around {
                [before2, Some(before), _, after, after2]
                    if starts_part(before2, before, current, [after, after2]) =>
                {
                    Some(start)
                }
                _ => None,
            });
    let mut parts = Vec::new();
    let mut from = 0;
    for start in starts {
        parts.extend(word.get(from..start));
        from = start;
    }
    parts.extend(word.get(from..));
    parts
}

/// Whether a camelCase part starts at `current`, the letter after `before2`
/// and `before` and before the two letters `after`: where a lowercase letter
/// or a digit meets an uppercase letter, and before the last letter of three
/// uppercase letters or more followed by two lowercase ones.
fn starts_part(
    before2: Option<char>,
    before: char,
    current: char,
    after: [Option<char>; 2],
) -> bool {
    let lowercase_after = after
        .iter()
        .all(|letter| letter.is_some_and(char::is_lowercase));
    current.is_uppercase()
        && (before.is_lowercase()
            || before.is_numeric()
            || (before.is_uppercase()
                && before2.is_some_and(char::is_uppercase)
                && lowercase_after))
}
