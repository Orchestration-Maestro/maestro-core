//! Current-source coverage, with pending command groups and literal-identity exceptions.
use super::lexer::{Token, tokens};
use crate::presentation::messages::BUILT_INS;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

/// Small, strict disposition rules; no line-number identities or copied key inventory.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rules {
    /// Command groups explicitly left to later slices.
    pending: Vec<Pending>,
    /// Audited machine prose identified by path and exact literal bytes.
    exceptions: Vec<Exception>,
}

/// A pending slice owns its command paths, not a frozen copy of its strings.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    /// The approved follow-up slice.
    slice: u8,
    /// Directory prefixes or exact Rust filenames, relative to cli.
    prefixes: Vec<String>,
}

/// An explicit exception cannot silently move to another source file or wording.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Exception {
    /// Current source path relative to cli.
    path: String,
    /// Exact source literal contents.
    literal: String,
    /// Why this is not translated UI text.
    reason: String,
}

/// Enumerate current Rust sources, including new files that Git has not staged yet.
fn sources(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// Find the end of a balanced Rust token group, including mixed nested delimiters.
fn group_end(tokens: &[Token<'_>], start: usize) -> usize {
    let mut depth = 0;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        if token.string {
            continue;
        }
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                depth -= 1;
                if depth == 0 {
                    return index;
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced source token group");
}

/// Mark syntactic exclusions; prose is never classified by its source line number.
fn exclusions(tokens: &[Token<'_>]) -> Vec<Option<&'static str>> {
    let mut result = vec![None; tokens.len()];
    for (index, token) in tokens.iter().enumerate() {
        if token.string {
            continue;
        }
        if token.text == "#" && tokens.get(index + 1).is_some_and(|next| next.text == "[") {
            let (end, reason) = attribute_range(tokens, index);
            result[index..=end].fill(Some(reason));
        }
        if tokens.get(index + 1).is_some_and(|next| next.text == "!") {
            let reason = match token.text {
                "json" => Some("machine/JSON value"),
                "trace" | "debug" | "info" | "warn" | "error" => Some("operational log"),
                _ => None,
            };
            if let Some(reason) = reason {
                let end = group_end(tokens, index + 2);
                mark(&mut result[index..=end], reason);
            }
        }
        if machine_call(tokens, index) && tokens.get(index + 1).is_some_and(|next| next.text == "(")
        {
            let end = group_end(tokens, index + 1);
            mark(&mut result[index..=end], "machine argument");
        }
    }
    result
}

/// A test attribute excludes its complete item, not just the attribute's strings.
fn attribute_range(tokens: &[Token<'_>], index: usize) -> (usize, &'static str) {
    let end = group_end(tokens, index + 1);
    let attribute: String = tokens[index + 2..end]
        .iter()
        .map(|token| token.text)
        .collect();
    if attribute != "cfg(test)" && attribute != "test" {
        return (end, "help/reference attribute");
    }
    let body = (end + 1..tokens.len())
        .find(|next| matches!(tokens[*next].text, "{" | ";"))
        .unwrap();
    let end = if tokens[body].text == "{" {
        group_end(tokens, body)
    } else {
        body
    };
    (end, "test-only")
}

/// More local syntax must not overwrite an enclosing test-only exclusion.
fn mark(items: &mut [Option<&'static str>], reason: &'static str) {
    for item in items {
        if item.is_none() {
            *item = Some(reason);
        }
    }
}

/// Literal keys, paths, patterns and language tags are data at these existing call boundaries.
fn machine_call(tokens: &[Token<'_>], index: usize) -> bool {
    let name = tokens[index].text;
    if matches!(
        name,
        "wording"
            | "with_message"
            | "join"
            | "var"
            | "var_os"
            | "contains"
            | "starts_with"
            | "ends_with"
            | "strip_prefix"
            | "strip_suffix"
            | "get"
            | "split_once"
    ) {
        return true;
    }
    index >= 3
        && ((name == "new" && tokens[index - 3].text == "Message")
            || (name == "select" && tokens[index - 3].text == "Interface"))
}

/// Placeholder-only formatting contains no authored human words.
fn only_formatting(text: &str) -> bool {
    let mut depth = 0;
    for character in text.chars() {
        match character {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ if depth == 0 && character.is_alphanumeric() => return false,
            _ => {}
        }
    }
    true
}

/// Machine schemas, filenames and struct status values retain their public contracts.
fn machine_literal(tokens: &[Token<'_>], index: usize) -> bool {
    let text = tokens[index].text;
    if only_formatting(text) {
        return true;
    }
    if text.starts_with("maestro-") || text.starts_with(".maestro-") {
        return true;
    }
    if !text.chars().any(char::is_whitespace)
        && (text.contains('/') || text.contains('\\') || text.contains('.'))
    {
        return true;
    }
    index >= 2
        && tokens[index - 1].text == ":"
        && matches!(tokens[index - 2].text, "schema" | "status")
}

/// Derive catalogue spelling from the actual typed identities used by current call sites.
fn message_name(variant: &str) -> String {
    let mut name = String::new();
    for (index, character) in variant.chars().enumerate() {
        if character.is_ascii_uppercase() && index != 0 {
            name.push('_');
        }
        name.push(character.to_ascii_lowercase());
    }
    name
}

#[test]
fn catalog_presentation_current_inventory_refuses_unclassified_literals_and_missing_keys() {
    let rules: Rules = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/catalog/presentation/inventory-rules.json"
    ))
    .unwrap();
    assert_eq!(
        rules
            .pending
            .iter()
            .map(|pending| pending.slice)
            .collect::<Vec<_>>(),
        [2, 3, 4]
    );
    assert!(
        rules
            .exceptions
            .iter()
            .all(|exception| !exception.reason.is_empty())
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cli");
    let mut unknown = Vec::new();
    let mut used_keys = BTreeSet::new();
    let mut counts = BTreeMap::new();
    let mut seen_exceptions = BTreeSet::new();
    for path in sources(&root) {
        let relative = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let source = fs::read_to_string(&path).unwrap();
        let tokens = tokens(&source);
        let excluded = exclusions(&tokens);
        for (index, token) in tokens.iter().enumerate() {
            if excluded[index] != Some("test-only")
                && !token.string
                && token.text == "MessageKey"
                && tokens.get(index + 1).is_some_and(|next| next.text == ":")
                && tokens.get(index + 2).is_some_and(|next| next.text == ":")
                && tokens.get(index + 3).is_some()
            {
                used_keys.insert(message_name(tokens[index + 3].text));
            }
            if !token.string {
                continue;
            }
            let reason = excluded[index]
                .or_else(|| machine_literal(&tokens, index).then_some("machine/JSON value"));
            let disposition = classify(&relative, token.text, reason, &rules, &mut seen_exceptions);
            match disposition {
                Some(reason) => *counts.entry(reason).or_insert(0) += 1,
                None => unknown.push(format!(
                    "{relative}:{}: {:?}",
                    source[..token.offset].lines().count() + 1,
                    token.text
                )),
            }
        }
    }
    assert!(
        unknown.is_empty(),
        "unclassified CLI literals:\n{}",
        unknown.join("\n")
    );
    assert_eq!(
        seen_exceptions.len(),
        rules.exceptions.len(),
        "remove stale exceptions"
    );
    for (language, source) in BUILT_INS {
        let catalog: Value = serde_json::from_str(source).unwrap();
        for key in &used_keys {
            assert!(
                catalog.get(key).is_some(),
                "missing migrated key {key} in {language}"
            );
        }
    }
    println!(
        "current literal dispositions: {counts:?}; {} typed keys",
        used_keys.len()
    );
}

/// Pending prose is visible, and a new literal outside its group must have a disposition.
fn classify<'a>(
    path: &str,
    literal: &str,
    excluded: Option<&'a str>,
    rules: &'a Rules,
    seen: &mut BTreeSet<usize>,
) -> Option<&'a str> {
    if path
        .split('/')
        .any(|part| part == "tests" || part == "tests.rs" || part.ends_with("_tests.rs"))
    {
        return Some("test-only");
    }
    if excluded == Some("test-only") {
        return excluded;
    }
    if let Some((number, _)) = rules
        .exceptions
        .iter()
        .enumerate()
        .find(|(_, exception)| exception.path == path && exception.literal == literal)
    {
        seen.insert(number);
        return Some("explicit exception");
    }
    if let Some(reason) = excluded {
        return Some(reason);
    }
    if let Some(pending) = rules.pending.iter().find(|pending| {
        pending
            .prefixes
            .iter()
            .any(|prefix| (prefix.ends_with('/') && path.starts_with(prefix)) || path == prefix)
    }) {
        return Some(match pending.slice {
            2 => "pending slice 2",
            3 => "pending slice 3",
            4 => "pending slice 4",
            _ => panic!("invalid pending slice"),
        });
    }
    None
}
