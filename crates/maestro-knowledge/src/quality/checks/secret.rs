//! `text.suspected-secret`: a credential anywhere in a revision's original
//! Markdown, its front matter and code included, in one of the formats whose
//! shape alone identifies it: secrets are quarantined before indexing
//! (docs/architecture/01 §3). The formats are narrow, so that the check is
//! right when it fires:
//!
//! - a PEM private key block: a `-----BEGIN` marker whose label, in capitals,
//!   digits and spaces, ends with `PRIVATE KEY` or `PRIVATE KEY BLOCK`, then
//!   at least 64 base64 characters on lines of base64 alone, blank lines and
//!   header lines (`Name: value`) apart, then an `-----END` marker; a line
//!   break written `\n`, as in a JSON string, is one;
//! - an AWS access key ID: `AKIA` or `ASIA`, then 16 capitals and digits from
//!   `2` to `7`;
//! - a GitHub token: `ghp_`, `gho_`, `ghu_`, `ghs_` or `ghr_`, then 36 letters
//!   and digits; or `github_pat_`, then 22 letters and digits, `_` and 59
//!   more;
//! - a GitLab token: `glpat-`, `gldt-` or `glrt-`, then at least 20 letters,
//!   digits, `-` and `_`;
//! - a Slack token: `xoxa-`, `xoxb-`, `xoxe-`, `xoxo-`, `xoxp-`, `xoxr-` or
//!   `xoxs-`, then two numbers of at least 10 digits, each followed by `-`,
//!   then at least 16 letters and digits.
//!
//! A token is a whole word: the characters on either side of it are none a
//! token holds, letters, digits, `-` and `_`. A token whose random part holds
//! `example`, in any case, 4 equal characters in a row, or 5 that each follow
//! the one before, such as `xxxx` or `12345`, is a documentation placeholder,
//! as is a key block whose body is not base64, such as `...`. The reason
//! names each format and the lines it is on, never the text.

use super::flag::{Flag, counted, number};
use maestro_kernel::document::Outcome;
use std::collections::{BTreeMap, BTreeSet};

/// What opens a PEM block, before its label.
const BEGIN: &str = "-----BEGIN ";

/// What closes a PEM block, before its label.
const END: &str = "-----END ";

/// What ends a PEM marker's label.
const DASHES: &str = "-----";

/// The fewest base64 characters a private key's body holds.
const KEY_CHARACTERS: usize = 64;

/// A credential format the check recognizes, in the order a reason names
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Format {
    /// A PEM private key block.
    PrivateKey,
    /// An AWS access key ID.
    Aws,
    /// A GitHub token.
    GitHub,
    /// A GitLab token.
    GitLab,
    /// A Slack token.
    Slack,
}

impl Format {
    /// Its name, as a reason gives it.
    fn name(self) -> &'static str {
        match self {
            Self::PrivateKey => "PEM private key block",
            Self::Aws => "AWS access key ID",
            Self::GitHub => "GitHub token",
            Self::GitLab => "GitLab token",
            Self::Slack => "Slack token",
        }
    }
}

/// `text.suspected-secret`, `quarantined`: a credential in one of the
/// formats of this module, anywhere in `markdown`, the original.
pub(super) fn suspected_secret(markdown: &str) -> Option<Flag> {
    let mut found: BTreeSet<(Format, usize)> = private_keys(markdown)
        .map(|line| (Format::PrivateKey, line))
        .collect();
    for (index, line) in markdown.lines().enumerate() {
        found.extend(
            line.split(|character: char| !in_token(character))
                .filter_map(token_format)
                .map(|format| (format, index + 1)),
        );
    }
    if found.is_empty() {
        return None;
    }
    let mut lines: BTreeMap<Format, Vec<String>> = BTreeMap::new();
    for (format, line) in &found {
        lines.entry(*format).or_default().push(line.to_string());
    }
    let named: Vec<String> = lines
        .into_iter()
        .map(|(format, lines)| {
            let noun = if lines.len() == 1 { "line" } else { "lines" };
            format!("{} ({noun} {})", format.name(), lines.join(", "))
        })
        .collect();
    Some(Flag {
        rule: "text.suspected-secret",
        outcome: Outcome::Quarantined,
        reason: format!(
            "{}, quarantined until someone reviews the document: {}",
            counted(number(found.len()), "suspected secret"),
            named.join("; ")
        ),
    })
}

/// Whether `character` is one a token holds.
fn in_token(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
}

/// The format of the token `word` is, if it is one.
fn token_format(word: &str) -> Option<Format> {
    if aws(word) {
        Some(Format::Aws)
    } else if github(word) {
        Some(Format::GitHub)
    } else if gitlab(word) {
        Some(Format::GitLab)
    } else if slack(word) {
        Some(Format::Slack)
    } else {
        None
    }
}

/// Whether `word` is an AWS access key ID.
fn aws(word: &str) -> bool {
    ["AKIA", "ASIA"]
        .into_iter()
        .find_map(|prefix| word.strip_prefix(prefix))
        .is_some_and(|random| {
            random.len() == 16
                && random
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || (b'2'..=b'7').contains(&byte))
                && !placeholder(random)
        })
}

/// Whether `word` is a GitHub token.
fn github(word: &str) -> bool {
    let classic = ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"]
        .into_iter()
        .find_map(|prefix| word.strip_prefix(prefix))
        .is_some_and(|random| {
            random.len() == 36
                && random.bytes().all(|byte| byte.is_ascii_alphanumeric())
                && !placeholder(random)
        });
    let fine_grained = word.strip_prefix("github_pat_").is_some_and(|random| {
        random.len() == 82
            && random.char_indices().all(|(index, character)| {
                if index == 22 {
                    character == '_'
                } else {
                    character.is_ascii_alphanumeric()
                }
            })
            && !placeholder(random)
    });
    classic || fine_grained
}

/// Whether `word` is a GitLab token.
fn gitlab(word: &str) -> bool {
    ["glpat-", "gldt-", "glrt-"]
        .into_iter()
        .find_map(|prefix| word.strip_prefix(prefix))
        .is_some_and(|random| random.len() >= 20 && !placeholder(random))
}

/// Whether `word` is a Slack token.
fn slack(word: &str) -> bool {
    let Some(random) = word
        .strip_prefix("xox")
        .and_then(|rest| rest.strip_prefix(['a', 'b', 'e', 'o', 'p', 'r', 's']))
        .and_then(|rest| rest.strip_prefix('-'))
    else {
        return false;
    };
    let mut parts = random.splitn(3, '-');
    let (Some(first), Some(second), Some(secret)) = (parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    slack_number(first)
        && slack_number(second)
        && secret.bytes().filter(u8::is_ascii_alphanumeric).count() >= 16
        && !placeholder(random)
}

/// Whether `part` is one of a Slack token's numbers: at least 10 digits.
fn slack_number(part: &str) -> bool {
    part.len() >= 10 && part.bytes().all(|byte| byte.is_ascii_digit())
}

/// Whether a token's random part, `random`, is a documentation placeholder:
/// it holds `example`, in any case, 4 equal characters in a row, or 5 that
/// each follow the one before, such as `xxxx` or `12345`.
fn placeholder(random: &str) -> bool {
    let bytes = random.as_bytes();
    random.to_ascii_lowercase().contains("example")
        || bytes
            .windows(4)
            .any(|run| run.iter().all(|byte| Some(byte) == run.first()))
        || bytes.windows(5).any(|run| {
            run.windows(2).all(
                |pair| matches!(pair, [first, second] if first.checked_add(1) == Some(*second)),
            )
        })
}

/// The lines on which a PEM private key block starts in `markdown`.
fn private_keys(markdown: &str) -> impl Iterator<Item = usize> + '_ {
    markdown
        .match_indices(BEGIN)
        .filter(|(start, _)| markdown.get(start + BEGIN.len()..).is_some_and(private_key))
        .map(|(start, _)| line_of(markdown, start))
}

/// Whether `block`, what follows a `-----BEGIN` marker, is a private key:
/// its label, `-----`, a key's body, then an `-----END` marker.
fn private_key(block: &str) -> bool {
    let Some((label, rest)) = block.split_once(DASHES) else {
        return false;
    };
    let Some((body, _)) = rest.split_once(END) else {
        return false;
    };
    label
        .bytes()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b' ')
        && (label.ends_with("PRIVATE KEY") || label.ends_with("PRIVATE KEY BLOCK"))
        && key_body(body)
}

/// Whether `body` is a key's: at least [`KEY_CHARACTERS`] base64 characters
/// on lines of base64 alone, blank lines and header lines apart, a line
/// break written `\n` being one.
fn key_body(body: &str) -> bool {
    let body = body.replace("\\n", "\n").replace("\\r", "");
    let mut characters = 0;
    for line in body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.contains(':'))
    {
        let base64 = line
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='));
        if !base64 {
            return false;
        }
        characters += line.len();
    }
    characters >= KEY_CHARACTERS
}

/// The line, from 1, of the byte `offset` of `markdown`.
fn line_of(markdown: &str, offset: usize) -> usize {
    markdown
        .bytes()
        .take(offset)
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}
