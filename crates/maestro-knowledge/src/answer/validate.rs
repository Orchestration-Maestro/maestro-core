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

/// A stable validation failure used only in the one repair instruction.
#[derive(Clone, Copy)]
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
) -> Result<Reply, ValidationFailure> {
    let reply = strip_thinking(raw)?;
    if reply == "NOT_FOUND" {
        return Ok(Reply::NotFound);
    }
    let (plain, mut citations) = remove_citations(reply)?;
    if citations.is_empty()
        || citations
            .iter()
            .any(|number| !bundle.passages.iter().any(|passage| passage.n == *number))
    {
        return Err(ValidationFailure::Citation);
    }
    citations.sort_unstable();
    citations.dedup();
    if plain
        .split_whitespace()
        .filter(|token| token.chars().any(char::is_alphabetic))
        .count()
        < 3
    {
        return Err(ValidationFailure::TooShort);
    }
    if unsupported_literals(&plain, &request.question, bundle) {
        return Err(ValidationFailure::UnsupportedLiteral);
    }
    Ok(Reply::Answer(ValidReply {
        text: reply.to_owned(),
        citations,
    }))
}

/// Removes strict `[n]` markers, refusing malformed or empty markers.
fn remove_citations(text: &str) -> Result<(String, Vec<u32>), ValidationFailure> {
    let mut plain = String::with_capacity(text.len());
    let mut citations = Vec::new();
    let mut remaining = text;
    while let Some(open) = remaining.find('[') {
        let Some(prefix) = remaining.get(..open) else {
            return Err(ValidationFailure::Citation);
        };
        plain.push_str(prefix);
        let after_open = remaining
            .get(open + 1..)
            .ok_or(ValidationFailure::Citation)?;
        let Some(close) = after_open.find(']') else {
            return Err(ValidationFailure::Citation);
        };
        let number = after_open
            .get(..close)
            .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|number| *number > 0)
            .ok_or(ValidationFailure::Citation)?;
        citations.push(number);
        plain.push(' ');
        remaining = after_open
            .get(close + 1..)
            .ok_or(ValidationFailure::Citation)?;
    }
    if remaining.contains(']') {
        return Err(ValidationFailure::Citation);
    }
    plain.push_str(remaining);
    Ok((plain, citations))
}

/// Rejects unsupported command, path, number, version, flag and error-code literals.
fn unsupported_literals(text: &str, question: &str, bundle: &Bundle) -> bool {
    let mut literals: BTreeSet<String> = understand(&without_fenced_code(text))
        .identifiers
        .into_iter()
        .filter(|identifier| identifier.family == Family::Command)
        .map(|identifier| identifier.text)
        .collect();
    literals.extend(literal_tokens(text));
    literals.extend(backtick_literals(text));
    let tokens = answer_tokens(text);
    for pair in tokens.windows(2) {
        let Some((command, flag)) = pair.first().zip(pair.get(1)) else {
            continue;
        };
        if is_flag(flag) && is_command_name(command) {
            literals.insert((*command).to_owned());
        }
    }
    literals.into_iter().any(|literal| {
        !contains_literal(question, &literal)
            && !bundle.passages.iter().any(|passage| {
                contains_literal(&passage.title, &literal)
                    || passage
                        .section_path
                        .iter()
                        .any(|section| contains_literal(section, &literal))
                    || contains_literal(&passage.text, &literal)
            })
    })
}

/// Extracts exact answer-side tokens with command, option, path or numeric syntax.
fn literal_tokens(text: &str) -> BTreeSet<String> {
    answer_tokens(text)
        .into_iter()
        .filter(|token| {
            token
                .chars()
                .any(|character| matches!(character, '_' | '$' | '=' | '/' | '\\'))
                || token.chars().any(|character| character.is_ascii_digit())
                || is_flag(token)
        })
        .map(str::to_owned)
        .collect()
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
    let mut start = 0;
    let mut end = token.len();
    while start < end {
        let Some(character) = token.get(start..end).and_then(|tail| tail.chars().next()) else {
            break;
        };
        if !edge_punctuation(character, token.get(start..end).unwrap_or_default(), true) {
            break;
        }
        start += character.len_utf8();
    }
    while start < end {
        let Some(character) = token
            .get(start..end)
            .and_then(|tail| tail.chars().next_back())
        else {
            break;
        };
        if !edge_punctuation(character, token.get(start..end).unwrap_or_default(), false) {
            break;
        }
        end -= character.len_utf8();
    }
    token.get(start..end).unwrap_or_default()
}

/// Keeps path and option syntax but drops quotes, brackets and sentence marks.
fn edge_punctuation(character: char, token: &str, leading: bool) -> bool {
    if matches!(character, '_' | '$' | '=' | '/' | '\\' | '-') {
        return false;
    }
    if character == '.' && leading && (token.starts_with("./") || token.starts_with("../")) {
        return false;
    }
    character.is_ascii_punctuation() || matches!(character, '“' | '”' | '‘' | '’' | '—' | '–')
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

/// Omits fenced blocks from command inference; their contents are checked line by line.
fn without_fenced_code(text: &str) -> String {
    let mut prose = String::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
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
        if trimmed.starts_with("```") {
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
    let mut literals = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = line.get(cursor..).and_then(|tail| tail.find('`')) {
        let Some(start) = cursor.checked_add(relative_start) else {
            break;
        };
        let delimiter_length = line
            .get(start..)
            .unwrap_or_default()
            .chars()
            .take_while(|character| *character == '`')
            .count();
        let Some(content_start) = start.checked_add(delimiter_length) else {
            break;
        };
        let mut search = content_start;
        let mut close = None;
        while let Some(relative_close) = line.get(search..).and_then(|tail| tail.find('`')) {
            let Some(close_start) = search.checked_add(relative_close) else {
                break;
            };
            let close_length = line
                .get(close_start..)
                .unwrap_or_default()
                .chars()
                .take_while(|character| *character == '`')
                .count();
            let Some(close_end) = close_start.checked_add(close_length) else {
                break;
            };
            if close_length == delimiter_length {
                close = Some((close_start, close_end));
                break;
            }
            search = close_end;
        }
        if let Some((close_start, close_end)) = close {
            if let Some(literal) = line.get(content_start..close_start) {
                literals.push(literal.to_owned());
            }
            cursor = close_end;
        } else {
            cursor = content_start;
        }
    }
    literals
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
