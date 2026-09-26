//! The quality ledger, `maestro-quality-ledger/1`: one strict JSON rule a
//! line, read whole or refused with the number of its first line that is no
//! rule; a missing ledger reads as an empty one.
#![cfg(test)]

use maestro_kernel::{artifact::Digest, document::Outcome};
use maestro_knowledge::quality::{Ledger, LedgerError};
use serde_json::{Value, json};
use std::{env, error::Error as _, fs, path::PathBuf, process};

/// A rule of the ledger, `id`, excluding the release 9.9 of the set `notes`.
fn rule(id: &str) -> Value {
    json!({
        "schema": "maestro-quality-ledger/1",
        "id": id,
        "match": { "set": "notes", "version": "9.9" },
        "disposition": "excluded",
        "reason": "release 10 supersedes release 9.9",
        "decided_by": "Ada",
        "reversal": "remove this rule; the revisions it decided keep their disposition",
    })
}

/// [`rule`] `id` with its field `key` set to `value`, or removed for null.
fn with(id: &str, key: &str, value: Value) -> Value {
    let mut rule = rule(id);
    let fields = rule.as_object_mut().unwrap();
    if value.is_null() {
        fields.remove(key);
    } else {
        fields.insert(key.to_owned(), value);
    }
    rule
}

/// The ledger whose lines are `lines`, each followed by a newline.
fn ledger(lines: &[String]) -> Result<Ledger, LedgerError> {
    lines
        .iter()
        .map(|line| format!("{line}\n"))
        .collect::<Vec<_>>()
        .concat()
        .parse()
}

#[test]
fn a_ledger_holds_one_rule_a_line_in_their_order() {
    let ledger = ledger(&[rule("first").to_string(), rule("second").to_string()]).unwrap();
    let ids: Vec<&str> = ledger.rules().iter().map(|rule| rule.id.as_str()).collect();
    assert_eq!(ids, ["first", "second"]);
    let first = &ledger.rules()[0];
    assert_eq!(first.disposition, Outcome::Excluded);
    assert_eq!(first.matches.set.as_deref(), Some("notes"));
    assert_eq!(first.matches.version.as_deref(), Some("9.9"));
    assert_eq!(first.matches.source_ref, None);
    assert_eq!(first.reason, "release 10 supersedes release 9.9");
    assert_eq!(first.decided_by, "Ada");
    assert!(first.reversal.starts_with("remove this rule"));
}

#[test]
fn each_disposition_and_each_field_of_a_corpus_entry_but_its_path_can_be_named() {
    let digest = Digest::of(b"page").as_str().to_owned();
    let every_field = json!({
        "source_ref": "https://example.org/page", "sha256": digest, "title": "Page",
        "source_kind": "guide", "set": "notes", "version": "9.9", "lang": "en",
        "captured_at": "2026-09-01T00:00:00Z", "product": "Relay", "component": "agent",
        "platform": "linux",
    });
    let lines: Vec<String> = [
        "accepted",
        "accepted_with_warnings",
        "needs_reextraction",
        "quarantined",
        "excluded",
    ]
    .into_iter()
    .map(|name| {
        let mut rule = with(name, "disposition", Value::from(name));
        rule["match"] = every_field.clone();
        rule.to_string()
    })
    .collect();
    let ledger = ledger(&lines).unwrap();
    let outcomes: Vec<Outcome> = ledger.rules().iter().map(|rule| rule.disposition).collect();
    assert_eq!(
        outcomes,
        [
            Outcome::Accepted,
            Outcome::AcceptedWithWarnings,
            Outcome::NeedsReextraction,
            Outcome::Quarantined,
            Outcome::Excluded,
        ]
    );
    let matches = &ledger.rules()[0].matches;
    assert_eq!(matches.sha256, Some(Digest::of(b"page")));
    assert_eq!(matches.platform.as_deref(), Some("linux"));
}

#[test]
fn an_empty_ledger_and_a_missing_one_hold_no_rule() {
    assert_eq!("".parse::<Ledger>().unwrap(), Ledger::default());
    let missing = env::temp_dir()
        .join(format!("maestro-knowledge-ledger-{}", process::id()))
        .join("quality")
        .join("ledger.jsonl");
    assert_eq!(Ledger::load(&missing).unwrap(), Ledger::default());
    assert!(Ledger::default().rules().is_empty());
}

#[test]
fn a_ledger_that_exists_but_cannot_be_read_is_refused() {
    let directory: PathBuf = env::temp_dir().join(format!(
        "maestro-knowledge-ledger-directory-{}",
        process::id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let refused = Ledger::load(&directory);
    fs::remove_dir(&directory).unwrap();
    let Err(error @ LedgerError::Io { .. }) = refused else {
        panic!("{refused:?}");
    };
    assert!(error.to_string().contains("cannot read"), "{error}");
    assert!(error.source().is_some());
}

#[test]
fn each_line_that_is_no_strict_rule_is_refused_with_its_number() {
    let valid = rule("valid").to_string();
    let texts = |values: &[Value]| -> Vec<String> { values.iter().map(Value::to_string).collect() };
    let mut broken: Vec<String> = texts(&[
        json!([]),
        with("id", "tags", json!(["policy_exception"])),
        with("id", "reversal", Value::Null),
        with("id", "schema", json!("maestro-quality-ledger/2")),
        with("id", "disposition", json!("held")),
        with("id", "disposition", json!({ "excluded": null })),
        with("id", "reason", json!("  ")),
        with("id", "decided_by", json!("")),
        with("id", "match", json!({ "path": "notes/page.md" })),
        with("id", "match", json!({ "set": null })),
        with("id", "match", json!({ "set": "" })),
        with("id", "match", json!({ "sha256": "ABC" })),
        with("id", "match", json!(["set", "notes"])),
        with("Superseded Release", "id", json!("Superseded Release")),
        with("id", "id", json!(7)),
    ]);
    broken.extend(
        [
            "not json",
            "",
            "{\"schema\":\"maestro-quality-ledger/1\"} {}",
            &valid.replacen("\"reason\":", "\"reason\":\"twice\",\"reason\":", 1),
            &valid.replacen("\"set\":", "\"set\":\"twice\",\"set\":", 1),
        ]
        .map(str::to_owned),
    );
    for line in broken {
        let refused = ledger(&[valid.clone(), line.clone()]);
        let Err(LedgerError::Json { line: 2, error }) = &refused else {
            panic!("{line}: {refused:?}");
        };
        let message = refused.as_ref().unwrap_err().to_string();
        assert!(message.starts_with("line 2 "), "{line}: {message}");
        assert!(message.contains(&error.to_string()), "{message}");
    }
}

#[test]
fn a_rule_that_matches_no_field_or_shares_an_id_is_refused() {
    let valid = rule("valid").to_string();
    let empty = with("empty", "match", json!({})).to_string();
    let refused = ledger(&[valid.clone(), empty]);
    assert!(
        matches!(refused, Err(LedgerError::EmptyMatch { line: 2 })),
        "{refused:?}"
    );
    let message = refused.unwrap_err().to_string();
    assert!(message.contains("matches no field"), "{message}");
    let refused = ledger(&[valid.clone(), rule("other").to_string(), valid]);
    let Err(LedgerError::DuplicateId { line: 3, id }) = &refused else {
        panic!("{refused:?}");
    };
    assert_eq!(id, "valid");
    let message = refused.unwrap_err().to_string();
    assert!(
        message.contains("line 3") && message.contains("`valid`"),
        "{message}"
    );
}

#[test]
fn a_ledger_is_read_from_its_file() {
    let directory: PathBuf =
        env::temp_dir().join(format!("maestro-knowledge-ledger-file-{}", process::id()));
    fs::create_dir_all(&directory).unwrap();
    let file = directory.join("ledger.jsonl");
    fs::write(&file, format!("{}\n", rule("only"))).unwrap();
    let read = Ledger::load(&file);
    fs::remove_dir_all(&directory).unwrap();
    assert_eq!(read.unwrap().rules().len(), 1);
}
