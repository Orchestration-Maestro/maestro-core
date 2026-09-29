//! The default question splitter: a part starts at a relation phrase, such
//! as "only after" or "seulement après", or at a conditional, such as "when"
//! or "si", from the per-language lists of `intent_words`. A phrase inside a
//! quote, an identifier or a capitalized name starts no part, and neither
//! does one that would leave a part with fewer than two words of its own.
//! Each later part starts with the question's topic, the noun phrase after
//! the first article of the first part, such as "control-m job", so that
//! "only after the third failure" still says of what.

use super::{
    intent_words::{LEXICONS, Lexicon},
    question_parts::{QuestionSplit, QuestionSplitter, Unsplit},
};
use crate::{lexical, query::Understood};
use std::{iter, ops::Range};

/// The most parts a question splits into.
pub(super) const MAX_PARTS: usize = 3;
/// The fewest words of its own a part holds.
const MIN_PART_WORDS: usize = 2;
/// The most words of a topic.
const MAX_TOPIC_WORDS: usize = 4;

/// Splits at the relation phrases and conditionals of every known language.
#[derive(Clone, Copy, Debug, Default)]
pub struct RelationSplitter;

impl QuestionSplitter for RelationSplitter {
    fn split(&self, question: &Understood) -> QuestionSplit {
        let text = question.normalized.as_str();
        let protected = protected_ranges(question);
        let tokens = tokens(text, &protected);
        let mut cuts = Vec::new();
        let mut unsplit = None;
        let mut openers = 0_usize;
        for index in 0..tokens.len() {
            if !starts_phrase(&tokens, index) {
                continue;
            }
            openers += 1;
            let start = cuts.last().copied().unwrap_or(0);
            let reason = if tokens.get(index).is_some_and(|token| token.protected) {
                Some(Unsplit::Protected)
            } else if cuts.len() + 1 >= MAX_PARTS {
                Some(Unsplit::PartCap)
            } else if content_words(tokens.get(start..index).unwrap_or_default()) < MIN_PART_WORDS
                || content_words(tokens.get(index..).unwrap_or_default()) < MIN_PART_WORDS
            {
                Some(Unsplit::ShortPart)
            } else {
                None
            };
            match reason {
                None => cuts.push(index),
                Some(reason) => {
                    unsplit.get_or_insert(reason);
                }
            }
        }
        if openers == 0 {
            unsplit = Some(Unsplit::NoRelation);
        }
        QuestionSplit {
            parts: parts(text, &tokens, &cuts),
            unsplit,
        }
    }
}

/// One whitespace-separated word of the question.
struct Token<'a> {
    /// Its bytes in the question.
    span: Range<usize>,
    /// Its letters, folded and lower-cased, without outer punctuation.
    word: String,
    /// As written, without outer punctuation.
    written: &'a str,
    /// Whether it lies in a quote, an identifier or a capitalized name.
    protected: bool,
}

/// The words of `text`, each marked when it overlaps `protected`.
fn tokens<'a>(text: &'a str, protected: &[Range<usize>]) -> Vec<Token<'a>> {
    let mut tokens = Vec::new();
    let mut offset = 0;
    for piece in text.split(' ') {
        let span = offset..offset + piece.len();
        offset = span.end + 1;
        let written = piece.trim_matches(|character: char| !is_word_character(character));
        if written.is_empty() {
            continue;
        }
        let capitalized =
            !tokens.is_empty() && written.chars().next().is_some_and(char::is_uppercase);
        tokens.push(Token {
            protected: capitalized
                || protected
                    .iter()
                    .any(|range| range.start < span.end && span.start < range.end),
            word: lexical::fold(written).to_lowercase(),
            written,
            span,
        });
    }
    tokens
}

/// Whether `character` belongs to a word: a letter, a digit or an inner
/// mark such as the apostrophe of "job's" or the dash of "control-m".
fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '\'' | '’' | '-' | '_')
}

/// The bytes of `question` inside quotes, or inside one of its identifiers.
fn protected_ranges(question: &Understood) -> Vec<Range<usize>> {
    let text = question.normalized.as_str();
    let mut ranges = Vec::new();
    let mut open: Option<(usize, char)> = None;
    for (index, character) in text.char_indices() {
        match (open, character) {
            (None, '"' | '`' | '“' | '«') => open = Some((index, character)),
            (Some((start, '"')), '"')
            | (Some((start, '`')), '`')
            | (Some((start, '“')), '”')
            | (Some((start, '«')), '»') => {
                ranges.push(start..index + character.len_utf8());
                open = None;
            }
            _ => {}
        }
    }
    if let Some((start, _)) = open {
        ranges.push(start..text.len());
    }
    for identifier in &question.identifiers {
        ranges.extend(
            text.match_indices(identifier.text.as_str())
                .map(|(start, found)| start..start + found.len()),
        );
    }
    ranges
}

/// Whether some language's `list` holds `word`.
fn listed(word: &str, list: fn(&Lexicon) -> &'static [&'static str]) -> bool {
    LEXICONS.iter().any(|lexicon| {
        list(lexicon)
            .iter()
            .any(|entry| lexical::fold(entry) == word)
    })
}

/// Whether `word` is a relation or a conditional.
fn is_opener(word: &str) -> bool {
    listed(word, |lexicon| lexicon.relations) || listed(word, |lexicon| lexicon.conditionals)
}

/// Whether the word at `index` starts a run of relation phrases and
/// conditionals, as "only" starts "only after".
fn starts_phrase(tokens: &[Token<'_>], index: usize) -> bool {
    tokens
        .get(index)
        .is_some_and(|token| is_opener(&token.word))
        && index
            .checked_sub(1)
            .and_then(|previous| tokens.get(previous))
            .is_none_or(|previous| !is_opener(&previous.word))
}

/// Whether `word` says something of its own: two letters or digits at
/// least, and not a stopword, an article, a relation, a conditional or a
/// question word. The pieces of an elided word count one by one.
fn is_content(word: &str) -> bool {
    word.split(['\'', '’']).any(|piece| {
        piece
            .chars()
            .filter(|character| character.is_alphanumeric())
            .count()
            >= 2
            && !lexical::is_stopword(piece)
            && !is_opener(piece)
            && !listed(piece, |lexicon| lexicon.articles)
            && !listed(piece, |lexicon| lexicon.question_words)
    })
}

/// How many of `tokens` say something of their own.
fn content_words(tokens: &[Token<'_>]) -> usize {
    tokens
        .iter()
        .filter(|token| is_content(&token.word))
        .count()
}

/// The parts `cuts` make of `text`, each later one led by the topic.
fn parts(text: &str, tokens: &[Token<'_>], cuts: &[usize]) -> Vec<String> {
    let Some(first_cut) = cuts.first() else {
        return vec![text.to_owned()];
    };
    let topic = topic(tokens.get(..*first_cut).unwrap_or_default());
    let starts = iter::once(0).chain(
        cuts.iter()
            .filter_map(|cut| tokens.get(*cut).map(|token| token.span.start)),
    );
    let ends = cuts
        .iter()
        .filter_map(|cut| tokens.get(*cut).map(|token| token.span.start))
        .chain(iter::once(text.len()));
    starts
        .zip(ends)
        .enumerate()
        .map(|(index, (start, end))| {
            let part = text.get(start..end).unwrap_or_default().trim();
            match &topic {
                Some(topic)
                    if index > 0
                        && !lexical::fold(part)
                            .to_lowercase()
                            .contains(&lexical::fold(topic).to_lowercase()) =>
                {
                    format!("{topic} {part}")
                }
                _ => part.to_owned(),
            }
        })
        .collect()
}

/// The noun phrase after the first article of `head`: the words up to the
/// next function word, at most [`MAX_TOPIC_WORDS`], with one that says
/// something of its own.
fn topic(head: &[Token<'_>]) -> Option<String> {
    let article = head
        .iter()
        .position(|token| listed(&token.word, |lexicon| lexicon.articles))?;
    let words = head
        .get(article + 1..)
        .unwrap_or_default()
        .iter()
        .take_while(|token| is_content(&token.word))
        .take(MAX_TOPIC_WORDS)
        .map(|token| token.written)
        .collect::<Vec<_>>();
    (!words.is_empty()).then(|| words.join(" "))
}
