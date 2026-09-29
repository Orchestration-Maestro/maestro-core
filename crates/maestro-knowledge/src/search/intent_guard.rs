//! The mechanical guard every expansion passes before it may vote.
//!
//! The passage and keywords only add dense and lexical votes, and the
//! reranker scores the original question, so the guard keeps what would
//! steer retrieval away from the question: its constraint words, its
//! quantities and its identifiers. Interface, platform and negation words
//! the question lacks are allowed; a number it lacks is not, because an
//! invented version, port or count steers the routes.

use super::{
    intent::{Expansion, ExpansionFailure},
    intent_words::{LEXICONS, Lexicon},
};
use crate::query::{Understood, understand};

/// The longest accepted passage, in UTF-8 bytes.
const MAX_PASSAGE: usize = 2048;
/// The longest accepted keyword text, in UTF-8 bytes.
const MAX_KEYWORDS: usize = 512;

/// A word as the guard compares it: a quantity in any spelling, or the
/// lower-case word.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    /// A numeral, a number word or an ordinal: "3", "3rd", "three",
    /// "third", as decimal digits without leading zeros.
    Quantity(String),
    /// Any other word.
    Word(String),
}

/// Accepts `expansion` of `question`, or names the first rule it breaks.
/// This is lexical protection, not a proof of equivalent meaning.
pub(super) fn validate(
    question: &Understood,
    expansion: &Expansion,
) -> Result<(), ExpansionFailure> {
    if expansion.passage.trim().is_empty() || expansion.keywords.trim().is_empty() {
        return Err(ExpansionFailure::Empty);
    }
    if expansion.passage.len() > MAX_PASSAGE || expansion.keywords.len() > MAX_KEYWORDS {
        return Err(ExpansionFailure::Oversize);
    }
    let original = tokens(&question.normalized);
    let passage = tokens(&expansion.passage);
    if !keeps_constraints(&original, &passage) {
        return Err(ExpansionFailure::ProtectedMissing);
    }
    let generated = tokens(&format!("{} {}", expansion.passage, expansion.keywords));
    if generated
        .iter()
        .any(|token| matches!(token, Token::Quantity(_)) && !original.contains(token))
    {
        return Err(ExpansionFailure::AddedNumber);
    }
    if question
        .identifiers
        .iter()
        .any(|identifier| !expansion.passage.contains(&identifier.text))
    {
        return Err(ExpansionFailure::IdentifierMissing);
    }
    let generated_identifiers =
        understand(&format!("{} {}", expansion.passage, expansion.keywords)).identifiers;
    if generated_identifiers.iter().any(|identifier| {
        !question
            .identifiers
            .iter()
            .any(|original| original.text == identifier.text)
    }) {
        return Err(ExpansionFailure::IdentifierAdded);
    }
    Ok(())
}

/// Whether every protected token of `original`, with the word a relation
/// binds, appears in `passage` in the same order.
fn keeps_constraints(original: &[Token], passage: &[Token]) -> bool {
    original.iter().enumerate().all(|(index, token)| {
        let width = match token {
            Token::Quantity(_) => 1,
            Token::Word(word) if is_in(word, |lexicon| lexicon.relations) => 2,
            Token::Word(word) if is_in(word, |lexicon| lexicon.constraints) => 1,
            Token::Word(_) => return true,
        };
        let phrase = original
            .get(index..(index + width).min(original.len()))
            .unwrap_or_default();
        passage.windows(phrase.len()).any(|window| window == phrase)
    })
}

/// The comparable tokens of `text`: case-folded whole words, so that `not`
/// never matches inside `notification`, without articles. An apostrophe
/// inside a word stays ("don't", "jusqu'à"), `’` counting as `'`; one at
/// either end is a quote mark, so `'not ok'` reads `not`. Other quote marks
/// already split words.
fn tokens(text: &str) -> Vec<Token> {
    text.replace('’', "'")
        .split(|character: char| !character.is_alphanumeric() && character != '\'')
        .map(|word| word.trim_matches('\''))
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .filter(|word| !is_in(word, |lexicon| lexicon.articles))
        .map(|word| quantity(&word).map_or(Token::Word(word), Token::Quantity))
        .collect()
}

/// The quantity `word` names, in decimal digits: a numeral, a numeral with
/// an ordinal ending, or a number word or ordinal of a known language.
fn quantity(word: &str) -> Option<String> {
    let digits = word
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(word.len());
    let (numeral, ending) = word.split_at(digits);
    if !numeral.is_empty()
        && (ending.is_empty() || is_in(ending, |lexicon| lexicon.ordinal_endings))
    {
        let significant = numeral.trim_start_matches('0');
        return Some(
            if significant.is_empty() {
                "0"
            } else {
                significant
            }
            .to_owned(),
        );
    }
    LEXICONS.iter().find_map(|lexicon| {
        lexicon
            .quantities
            .iter()
            .find(|(name, _)| *name == word)
            .map(|(_, value)| value.to_string())
    })
}

/// Whether `word` is in the list `list` picks from any known language.
fn is_in(word: &str, list: impl Fn(&Lexicon) -> &'static [&'static str]) -> bool {
    LEXICONS.iter().any(|lexicon| list(lexicon).contains(&word))
}
