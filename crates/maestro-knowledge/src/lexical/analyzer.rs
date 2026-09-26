//! The profile's name and the terms of a text.

use super::fold::fold;
use super::split::{Piece, pieces};
use super::stem::stem;
use super::stopwords::is_stopword;

/// The profile a generation records with its sparse vectors.
pub const PROFILE: &str = "bm25-en-fr/1";

/// The terms of `text`, in the order the analyzer finds them: an
/// identifier's whole form before its words, and a camelCase word before its
/// parts.
#[must_use]
pub fn terms(text: &str) -> Vec<String> {
    pieces(&fold(text))
        .into_iter()
        .filter_map(|piece| match piece {
            Piece::Identifier(whole) => Some(whole.to_lowercase()),
            Piece::Word(word) => {
                let word = word.to_lowercase();
                (!is_stopword(&word)).then(|| stem(&word))
            }
        })
        .collect()
}
