//! Rule-based query-kind classification in English and French.

use super::identifier_patterns::is_word_character;
use super::identifier_types::{Family, Identifier};
use crate::lexical;

/// The first matching rule for the query, or a lexical lookup when none match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryKind {
    /// A failure, crash, or error-code query.
    Troubleshooting,
    /// A query asking to compare alternatives.
    Comparison,
    /// A query requesting an exhaustive list or count.
    Global,
    /// A query asking for steps or configuration instructions.
    Procedure,
    /// A query asking for a definition or explanation.
    Concept,
    /// A query with no stronger rule match.
    Lookup,
}

/// English and French whole-word triggers for troubleshooting queries.
const TROUBLESHOOTING: &[&str] = &[
    "error",
    "fails",
    "failed",
    "crash",
    "not working",
    "erreur",
    "echoue",
    "plante",
    "ne fonctionne pas",
];
/// English and French whole-word triggers for comparison queries.
const COMPARISON: &[&str] = &[
    "difference between",
    "vs",
    "versus",
    "compare",
    "difference entre",
    "comparer",
];
/// English and French whole-word triggers for exhaustive list or count queries.
const GLOBAL: &[&str] = &[
    "list all",
    "all the",
    "how many",
    "combien",
    "tous les",
    "liste des",
];
/// English and French whole-word triggers for procedural queries.
const PROCEDURE: &[&str] = &[
    "how to",
    "how do i",
    "steps",
    "configure",
    "install",
    "set up",
    "comment",
    "etapes",
    "configurer",
    "installer",
];
/// English and French whole-word triggers for concept queries.
const CONCEPT: &[&str] = &[
    "what is",
    "what are",
    "explain",
    "meaning",
    "qu'est-ce que",
    "c'est quoi",
    "expliquer",
    "signifie",
];

/// Classifies the query after folding case and French accents.
pub(super) fn classify(text: &str, identifiers: &[Identifier]) -> QueryKind {
    let folded = lexical::fold(text)
        .to_lowercase()
        .replace(['\u{2019}', '\u{02bc}'], "'");
    if identifiers
        .iter()
        .any(|identifier| identifier.family == Family::ErrorCode)
        || contains_any(&folded, TROUBLESHOOTING)
    {
        QueryKind::Troubleshooting
    } else if contains_any(&folded, COMPARISON) {
        QueryKind::Comparison
    } else if contains_any(&folded, GLOBAL) {
        QueryKind::Global
    } else if contains_any(&folded, PROCEDURE) {
        QueryKind::Procedure
    } else if contains_any(&folded, CONCEPT) {
        QueryKind::Concept
    } else {
        QueryKind::Lookup
    }
}

/// Checks each rule against folded text as a whole phrase.
fn contains_any(text: &str, rules: &[&str]) -> bool {
    rules.iter().any(|rule| contains_whole_phrase(text, rule))
}

/// Finds a phrase only when neither edge touches another word character.
fn contains_whole_phrase(text: &str, phrase: &str) -> bool {
    text.match_indices(phrase).any(|(start, matched)| {
        let end = start + matched.len();
        !text
            .get(..start)
            .and_then(|before| before.chars().next_back())
            .is_some_and(is_word_character)
            && !text
                .get(end..)
                .and_then(|after| after.chars().next())
                .is_some_and(is_word_character)
    })
}
