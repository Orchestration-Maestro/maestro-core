//! What an import reports: its counts and refusals as JSON, the same counts
//! journaled as `maestro.knowledge.import.completed.v1`, and the refusals
//! that stop it before any work, for a caller who cannot read what it would
//! write or a manifest that cannot be found or read, each saying why.

use super::support::{Scratch, bindings, declaration, events, everything, line, markdown, page};
use maestro_kernel::{
    artifact::Digest,
    binding::{self, Bindings},
    document, journal,
    scope::{Right, ScopeSet},
    store::{self, Database},
};
use maestro_knowledge::import::{self, Error, Reason, Refusal};
use serde_json::{Value, json};
use std::error;

/// Grants `principal` the read right on `scope` alone, and returns what it
/// sees.
fn granted(database: &Database, principal: &str, scope: &str) -> ScopeSet {
    database
        .grant(principal, &scope.parse().unwrap(), Right::Read, "test")
        .unwrap();
    database.visible(principal).unwrap()
}

#[test]
fn the_counts_are_reported_as_json_and_journaled_as_import_completed() {
    let scratch = Scratch::new();
    let [compost, older, newer] = ["compost", "pruning, 2025", "pruning, 2026"].map(markdown);
    scratch.put("compost.md", &compost);
    scratch.put("pruning-2025.md", &older);
    scratch.put("pruning.md", &newer);
    scratch.manifest(&[
        line("compost.md", &compost, &page("compost")),
        line("pruning-2025.md", &newer, &page("pruning-2025")),
        line("pruning-2025.md", &older, &page("pruning")),
        line("pruning.md", &newer, &page("pruning")),
    ]);
    let database = scratch.database();
    let report = super::support::import(&scratch, &database, "garden");
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        json!({
            "collection": "garden",
            "imported": 1,
            "unchanged": 0,
            "held": 2,
            "refused": 1,
            "refusals": [{
                "source": "docs",
                "line": 2,
                "reason": "digest_mismatch",
                "declared": Digest::of(&newer).as_str(),
                "found": Digest::of(&older).as_str(),
            }],
        })
    );
    let journaled = events(&database, "garden");
    let types: Vec<&str> = journaled
        .iter()
        .map(|event| event.r#type.as_str())
        .collect();
    assert_eq!(
        types,
        [
            "maestro.knowledge.revision.held.v1",
            "maestro.knowledge.revision.held.v1",
            "maestro.knowledge.import.completed.v1",
        ]
    );
    let completed = &journaled[2];
    assert_eq!(completed.subject, "collection/garden");
    assert_eq!(completed.scope, "workspace/default/collection/garden");
    assert_eq!(
        completed.data,
        json!({ "collection": "garden", "imported": 1, "unchanged": 0, "held": 2, "refused": 1 })
    );
}

#[test]
fn each_refusal_is_reported_as_json_under_its_reason() {
    let digest = |text: &str| Digest::of(text.as_bytes());
    let message = || "why".to_owned();
    let reasons = [
        (
            Reason::Malformed { message: message() },
            json!({ "reason": "malformed", "message": "why" }),
        ),
        (
            Reason::Unreadable {
                path: "a.md".to_owned(),
                message: message(),
            },
            json!({ "reason": "unreadable", "path": "a.md", "message": "why" }),
        ),
        (
            Reason::DigestMismatch {
                declared: digest("a"),
                found: digest("b"),
            },
            json!({
                "reason": "digest_mismatch",
                "declared": digest("a").as_str(),
                "found": digest("b").as_str(),
            }),
        ),
        (
            Reason::SizeMismatch {
                declared: 2,
                found: 1,
            },
            json!({ "reason": "size_mismatch", "declared": 2, "found": 1 }),
        ),
        (Reason::NotUtf8, json!({ "reason": "not_utf8" })),
        (
            Reason::Canonicalization { message: message() },
            json!({ "reason": "canonicalization", "message": "why" }),
        ),
        (
            Reason::Conflict { message: message() },
            json!({ "reason": "conflict", "message": "why" }),
        ),
    ];
    for (reason, fields) in reasons {
        let refusal = Refusal {
            source: "docs".to_owned(),
            line: 7,
            reason,
        };
        let mut expected = json!({ "source": "docs", "line": 7 });
        let (Value::Object(expected_fields), Value::Object(reason_fields)) =
            (&mut expected, fields)
        else {
            panic!("objects");
        };
        expected_fields.extend(reason_fields);
        assert_eq!(serde_json::to_value(&refusal).unwrap(), expected);
    }
}

#[test]
fn an_import_the_caller_cannot_read_is_refused_before_anything_is_recorded() {
    let scratch = Scratch::new();
    let compost = markdown("compost");
    scratch.put("compost.md", &compost);
    scratch.manifest(&[line("compost.md", &compost, &page("compost"))]);
    let database = scratch.database();
    let elsewhere = granted(
        &database,
        "neighbour",
        "workspace/default/collection/orchard",
    );
    let refused = import::import(
        &database,
        &elsewhere,
        &declaration("garden"),
        &bindings(&scratch),
    );
    assert!(
        matches!(&refused, Err(Error::NotVisible(scope))
            if scope == "workspace/default/collection/garden/source/docs"),
        "{refused:?}"
    );
    let audit = everything(&database);
    assert_eq!(database.collection(&audit, "garden").unwrap(), None);
    assert!(events(&database, "garden").is_empty());
    let source = granted(
        &database,
        "gardener",
        "workspace/default/collection/garden/source/docs",
    );
    let report = import::import(
        &database,
        &source,
        &declaration("garden"),
        &bindings(&scratch),
    );
    assert_eq!(
        report.unwrap().imported,
        1,
        "the source's own scope suffices"
    );
}

#[test]
fn an_unbound_or_missing_manifest_stops_the_import() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let scopes = everything(&database);
    let unbound = import::import(
        &database,
        &scopes,
        &declaration("garden"),
        &Bindings::default(),
    );
    assert!(
        matches!(&unbound, Err(Error::Binding(binding::Error::Missing(name)))
            if name == "corpus_root"),
        "{unbound:?}"
    );
    assert_eq!(database.collection(&scopes, "garden").unwrap(), None);
    let missing = import::import(
        &database,
        &scopes,
        &declaration("garden"),
        &bindings(&scratch),
    );
    assert!(
        matches!(&missing, Err(Error::Manifest { source_id, .. }) if source_id == "docs"),
        "{missing:?}"
    );
    assert!(
        events(&database, "garden").is_empty(),
        "an import that stopped completes nothing"
    );
    for stopped in [unbound.unwrap_err(), missing.unwrap_err()] {
        assert!(error::Error::source(&stopped).is_some(), "{stopped}");
        assert!(!stopped.to_string().is_empty());
    }
}

#[test]
fn every_stop_says_why_and_keeps_its_cause() {
    let scope = "workspace/default/collection/garden/source/docs";
    let hidden = Error::NotVisible(scope.to_owned());
    assert!(hidden.to_string().contains(scope), "{hidden}");
    assert!(error::Error::source(&hidden).is_none());
    let unknown = || store::Error::UnknownMigration("0099_future".to_owned());
    let failures = [
        Error::Records(document::Error::DocumentConflict("doc-a".to_owned())),
        Error::Artifacts(unknown()),
        Error::Journal(journal::Error::Store(unknown())),
    ];
    let messages: Vec<String> = failures.iter().map(ToString::to_string).collect();
    for (failure, message) in failures.iter().zip(&messages) {
        let cause = error::Error::source(failure).unwrap().to_string();
        assert!(message.ends_with(&cause), "{message} carries {cause}");
    }
    let prefixes: Vec<&str> = messages
        .iter()
        .map(|message| message.split(':').next().unwrap())
        .collect();
    assert_eq!(
        prefixes,
        [
            "the kernel's records failed",
            "the artifact store failed",
            "the journal failed"
        ]
    );
}
