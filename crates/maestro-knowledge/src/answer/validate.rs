//! Deterministic support checks for buffered answerer replies.

use super::types::AskRequest;
use crate::{
    lexical::is_stopword,
    query::{Family, understand},
};
use maestro_kernel::evidence::Bundle;
use std::collections::BTreeSet;

/// A buffered model response, checked or explicitly not found.
pub(super) enum Reply {
    /// The model returned the exact refusal marker.
    NotFound,
    /// The answer passed the deterministic support checks.
    Answer(ValidReply),
}

/// The answer text and passage numbers that survived host validation.
pub(super) struct ValidReply {
    /// Buffered answer text, including its checked citation markers.
    pub(super) text: String,
    /// Sorted distinct passage numbers copied from valid markers.
    pub(super) citations: Vec<u32>,
}

/// A rejected reply: the check it failed and the offending tokens, if any.
pub(super) struct Invalid {
    /// The failed check.
    pub(super) failure: ValidationFailure,
    /// The reply's tokens that failed it, for the local explanation.
    pub(super) tokens: Vec<String>,
}

impl From<ValidationFailure> for Invalid {
    fn from(failure: ValidationFailure) -> Self {
        Self {
            failure,
            tokens: Vec::new(),
        }
    }
}

/// A stable validation failure used only in the one repair instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ValidationFailure {
    /// The reply contained malformed or leaked thinking markup.
    ThinkMarkup,
    /// The reply lacked a valid citation marker.
    Citation,
    /// The answer text was too short to be useful.
    TooShort,
    /// A command-like or numeric literal was unsupported by question/evidence.
    UnsupportedLiteral,
    /// The gateway could not produce a complete answer.
    InvalidAnswer,
}

impl ValidationFailure {
    /// A stable short code placed in the repair instruction.
    pub(super) fn code(self) -> &'static str {
        match self {
            Self::ThinkMarkup => "thinking_markup",
            Self::Citation => "invalid_citation",
            Self::TooShort => "too_short",
            Self::UnsupportedLiteral => "unsupported_literal",
            Self::InvalidAnswer => "invalid_answer",
        }
    }
}

/// Removes a leading thinking block before any text can reach the user.
fn strip_thinking(reply: &str) -> Result<&str, ValidationFailure> {
    let reply = reply.trim();
    let reply = if let Some(thinking) = reply.strip_prefix("<think>") {
        let Some((_, answer)) = thinking.split_once("</think>") else {
            return Err(ValidationFailure::ThinkMarkup);
        };
        answer.trim()
    } else {
        reply
    };
    if reply.contains("<think>") || reply.contains("</think>") {
        Err(ValidationFailure::ThinkMarkup)
    } else {
        Ok(reply)
    }
}

/// Parses citation markers and checks the answer's deterministic support guards.
pub(super) fn validate_reply(
    raw: &str,
    request: &AskRequest,
    bundle: &Bundle,
) -> Result<Reply, Invalid> {
    let reply = strip_thinking(raw)?;
    if reply == "NOT_FOUND" {
        return Ok(Reply::NotFound);
    }
    let (Cited { plain, prose }, mut citations) =
        remove_citations(reply).map_err(|marker| Invalid {
            failure: ValidationFailure::Citation,
            tokens: vec![marker],
        })?;
    let unknown: Vec<String> = citations
        .iter()
        .filter(|number| !bundle.passages.iter().any(|passage| passage.n == **number))
        .map(|number| format!("[{number}]"))
        .collect();
    if citations.is_empty() || !unknown.is_empty() {
        return Err(Invalid {
            failure: ValidationFailure::Citation,
            tokens: unknown,
        });
    }
    citations.sort_unstable();
    citations.dedup();
    if prose
        .split_whitespace()
        .filter(|token| token.chars().any(char::is_alphabetic))
        .count()
        < 3
    {
        return Err(ValidationFailure::TooShort.into());
    }
    let unsupported = unsupported_literals(&plain, &request.question, bundle);
    if !unsupported.is_empty() {
        return Err(Invalid {
            failure: ValidationFailure::UnsupportedLiteral,
            tokens: unsupported,
        });
    }
    Ok(Reply::Answer(ValidReply {
        text: reply.to_owned(),
        citations,
    }))
}

/// A reply's text once its citation markers are removed.
struct Cited {
    /// The text with bracketed text kept, for the literal check.
    plain: String,
    /// The text outside every bracket, for the length check.
    prose: String,
}

/// Removes citation markers, `[n]` or `[n, m]`, refusing a malformed or
/// empty one, which it returns. A bracket holding a digit is a marker, so
/// `[passage 9]` is refused; one holding letters and no digit, such as a
/// link's text, is text and stays in `plain`, but not in `prose`.
fn remove_citations(text: &str) -> Result<(Cited, Vec<u32>), String> {
    let mut plain = String::with_capacity(text.len());
    let mut prose = String::with_capacity(text.len());
    let mut citations = Vec::new();
    let mut remaining = text;
    while let Some(open) = remaining.find('[') {
        let before = remaining.get(..open).unwrap_or_default();
        plain.push_str(before);
        prose.push_str(before);
        let after_open = remaining.get(open + 1..).unwrap_or_default();
        let Some(close) = after_open.find(']') else {
            return Err(marker(after_open, after_open.len()));
        };
        let content = after_open.get(..close).unwrap_or_default();
        if content.chars().any(char::is_alphabetic)
            && !content.chars().any(|character| character.is_ascii_digit())
        {
            plain.push('[');
            plain.push_str(content);
            plain.push(']');
        } else {
            citations.extend(marker_numbers(content).ok_or_else(|| marker(after_open, close))?);
            plain.push(' ');
        }
        prose.push(' ');
        remaining = after_open.get(close + 1..).unwrap_or_default();
    }
    if remaining.contains(']') {
        return Err("]".to_owned());
    }
    plain.push_str(remaining);
    prose.push_str(remaining);
    Ok((Cited { plain, prose }, citations))
}

/// The passage numbers of a marker's `content`: numbers from 1, separated
/// by commas.
fn marker_numbers(content: &str) -> Option<Vec<u32>> {
    content
        .split(',')
        .map(|number| {
            let number = number.trim();
            number
                .bytes()
                .all(|byte| byte.is_ascii_digit())
                .then(|| number.parse::<u32>().ok())
                .flatten()
                .filter(|number| *number > 0)
        })
        .collect()
}

/// The malformed marker whose content is `after_open`'s first `length`
/// bytes, at most 24 characters of it.
fn marker(after_open: &str, length: usize) -> String {
    let content: String = after_open
        .get(..length)
        .unwrap_or_default()
        .chars()
        .take(24)
        .collect();
    format!("[{content}]")
}

/// The command, path, number, version, flag and error-code literals that
/// neither the question nor a passage supports. Both sides are compared
/// [`unescaped`].
fn unsupported_literals(raw: &str, question: &str, bundle: &Bundle) -> Vec<String> {
    let text = unescaped(raw);
    let mut literals: BTreeSet<String> = understand(&without_fenced_code(&text))
        .identifiers
        .into_iter()
        .filter(|identifier| identifier.family == Family::Command)
        .map(|identifier| identifier.text)
        .collect();
    literals.extend(literal_tokens(raw, &text));
    literals.extend(backtick_literals(&text));
    let tokens = answer_tokens(&text);
    for pair in tokens.windows(2) {
        let Some((command, flag)) = pair.first().zip(pair.get(1)) else {
            continue;
        };
        if is_flag(flag) && is_command_name(command) {
            literals.insert((*command).to_owned());
        }
    }
    let sources: Vec<String> = [question]
        .into_iter()
        .chain(bundle.passages.iter().flat_map(|passage| {
            [passage.title.as_str(), passage.text.as_str()]
                .into_iter()
                .chain(passage.section_path.iter().map(String::as_str))
        }))
        .map(unescaped)
        .collect();
    literals
        .into_iter()
        .filter(|literal| {
            !sources
                .iter()
                .any(|source| contains_literal(source, literal))
        })
        .collect()
}

/// `text` as its reader sees it: HTML entities decoded, and no backslash
/// escaping punctuation, as Markdown escapes it in passages and a model
/// copies the prompt's JSON escapes (`\"`, `\\_`) into its answer.
fn unescaped(text: &str) -> String {
    let mut current = [
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&amp;", "&"),
    ]
    .into_iter()
    .fold(text.to_owned(), |text, (entity, character)| {
        text.replace(entity, character)
    });
    // Each pass that changes the text drops a backslash, so the backslash
    // count bounds the passes.
    let passes = current.matches('\\').count();
    for _ in 0..passes {
        let next = without_escapes(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

/// Drops each backslash that escapes an ASCII punctuation character, once.
fn without_escapes(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        let escaped = (character == '\\')
            .then(|| characters.next_if(char::is_ascii_punctuation))
            .flatten();
        plain.push(escaped.unwrap_or(character));
    }
    plain
}

/// Extracts exact answer-side tokens with command, option, path or numeric
/// syntax, in their `unescaped` form. A token is one when its raw or its
/// unescaped form has that syntax, so a backslash that was a path's only
/// marker, as in `%USERPROFILE%\.toolrc`, still makes it a literal.
/// Unescaping never touches whitespace, so the two splits pair up.
fn literal_tokens(raw: &str, unescaped: &str) -> BTreeSet<String> {
    raw.split_whitespace()
        .zip(unescaped.split_whitespace())
        .filter(|(raw, clean)| {
            is_literal(trim_token_edges(raw)) || is_literal(trim_token_edges(clean))
        })
        .map(|(_, clean)| trim_token_edges(clean))
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Whether `token` has command, option, path or numeric syntax.
fn is_literal(token: &str) -> bool {
    token
        .chars()
        .any(|character| matches!(character, '_' | '$' | '=' | '/' | '\\'))
        || token.chars().any(|character| character.is_ascii_digit())
        || is_flag(token)
}

/// Returns nonempty whitespace tokens with sentence punctuation and wrappers removed.
fn answer_tokens(text: &str) -> Vec<&str> {
    text.split_whitespace()
        .map(trim_token_edges)
        .filter(|token| !token.is_empty())
        .collect()
}

/// Removes surrounding punctuation while preserving characters inside literals.
fn trim_token_edges(token: &str) -> &str {
    let start = token
        .char_indices()
        .find(|(index, character)| {
            !edge_punctuation(*character, token.get(*index..).unwrap_or_default(), true)
        })
        .map_or(token.len(), |(index, _)| index);
    let rest = token.get(start..).unwrap_or_default();
    let end = rest
        .char_indices()
        .rev()
        .find(|(_, character)| !edge_punctuation(*character, rest, false))
        .map_or(0, |(index, character)| index + character.len_utf8());
    rest.get(..end).unwrap_or_default()
}

/// Keeps path and option syntax but drops quotes, brackets and sentence marks.
fn edge_punctuation(character: char, token: &str, leading: bool) -> bool {
    if matches!(character, '_' | '$' | '=' | '/' | '\\' | '-') {
        return false;
    }
    if character == '.' && leading && (token.starts_with("./") || token.starts_with("../")) {
        return false;
    }
    character.is_ascii_punctuation()
        || matches!(character, '“' | '”' | '‘' | '’' | '—' | '–' | '«' | '»')
}

/// Recognizes one or two leading hyphens followed by an ASCII letter.
fn is_flag(token: &str) -> bool {
    token
        .strip_prefix("--")
        .or_else(|| token.strip_prefix('-'))
        .and_then(|name| name.chars().next())
        .is_some_and(|character| character.is_ascii_alphabetic())
}

/// Names a non-stopword immediately before a flag as the command it invokes.
fn is_command_name(token: &str) -> bool {
    let folded = token.to_lowercase();
    token.chars().any(char::is_alphabetic)
        && !is_stopword(&folded)
        && !matches!(
            folded.as_str(),
            "command"
                | "option"
                | "switch"
                | "flag"
                | "run"
                | "use"
                | "set"
                | "execute"
                | "invoke"
                | "pass"
                | "with"
                | "to"
        )
}

/// Whether `line` opens or closes a fenced block: three or more backticks
/// and an info string without a backtick, as Markdown requires.
fn is_fence(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("```") && !trimmed.trim_start_matches('`').contains('`')
}

/// Omits fenced blocks from command inference; their contents are checked line by line.
fn without_fenced_code(text: &str) -> String {
    let mut prose = String::new();
    let mut fenced = false;
    for line in text.lines() {
        if is_fence(line) {
            fenced = !fenced;
        } else if !fenced {
            prose.push_str(line);
            prose.push('\n');
        }
    }
    prose
}

/// Extracts code spans and each nonblank line inside fenced code blocks.
fn backtick_literals(text: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if is_fence(trimmed) {
            fenced = !fenced;
            continue;
        }
        if fenced {
            if !trimmed.is_empty() {
                literals.push(trimmed.to_owned());
            }
        } else {
            literals.extend(inline_backtick_literals(line));
        }
    }
    literals
}

/// Extracts complete inline spans delimited by equal runs of one or two backticks.
fn inline_backtick_literals(line: &str) -> Vec<String> {
    let runs = backtick_runs(line);
    let mut literals = Vec::new();
    let mut remaining = runs.as_slice();
    while let Some((&(open_start, open_end), rest)) = remaining.split_first() {
        let length = open_end.saturating_sub(open_start);
        remaining = match rest
            .iter()
            .position(|&(start, end)| end.saturating_sub(start) == length)
        {
            Some(close) => {
                let close_start = rest.get(close).map_or(open_end, |&(start, _)| start);
                literals.extend(line.get(open_end..close_start).map(str::to_owned));
                rest.get(close + 1..).unwrap_or_default()
            }
            None => rest,
        };
    }
    literals
}

/// The byte ranges of `line`'s maximal backtick runs, in order.
fn backtick_runs(line: &str) -> Vec<(usize, usize)> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (index, _) in line.match_indices('`') {
        match runs.last_mut() {
            Some((_, end)) if *end == index => *end = index + 1,
            _ => runs.push((index, index + 1)),
        }
    }
    runs
}

/// Checks a literal as the same whole-token sequence used on answer and evidence.
fn contains_literal(text: &str, literal: &str) -> bool {
    let expected = answer_tokens(literal);
    !expected.is_empty()
        && answer_tokens(text)
            .windows(expected.len())
            .any(|tokens| tokens == expected)
}

#[cfg(test)]
mod tests;
