//! Which revisions a preparation reads: those the quality gate accepted, with
//! or without warnings, and no other; and only for a caller who reads the
//! whole collection.

use super::scratch::{
    COLLECTION, Scratch, chunk_set_of, chunks_of, decide, decide_all, document_of, manifest_of,
    revision_of, tokenizer, words,
};
use crate::prepare::{Error, Ineligibility, LeftOut, Refusal, prepare};
use maestro_kernel::{
    document::{Document, Outcome, Revision},
    scope::Right,
};
use serde_json::json;
use std::collections::BTreeSet;

/// The documents of the corpus, one per outcome of the quality gate and one
/// left without a disposition.
const PATHS: [&str; 6] = [
    "accepted.md",
    "warned.md",
    "reextract.md",
    "quarantined.md",
    "excluded.md",
    "undecided.md",
];

#[test]
fn only_revisions_accepted_with_or_without_warnings_are_prepared() {
    let scratch = Scratch::new();
    let documents: Vec<(&str, String)> = PATHS
        .iter()
        .map(|path| {
            (
                *path,
                format!("# {path}\n\n{}\n", words(path.trim_end_matches(".md"), 40)),
            )
        })
        .collect();
    let borrowed: Vec<(&str, &str)> = documents
        .iter()
        .map(|(path, markdown)| (*path, markdown.as_str()))
        .collect();
    scratch.corpus(&borrowed);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    let outcomes = [
        Outcome::Accepted,
        Outcome::AcceptedWithWarnings,
        Outcome::NeedsReextraction,
        Outcome::Quarantined,
        Outcome::Excluded,
    ];
    for (path, outcome) in PATHS.iter().zip(outcomes) {
        decide(&database, &revision_of(&database, &scopes, path), outcome);
    }
    let (_, tokenizer) = tokenizer();
    let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    let eligible: BTreeSet<String> = ["accepted.md", "warned.md"]
        .iter()
        .map(|path| revision_of(&database, &scopes, path))
        .collect();
    assert_eq!(
        [report.eligible, report.prepared, report.refused],
        [2, 2, 0]
    );
    let chunked: BTreeSet<String> = chunks_of(&database, &scopes, &report)
        .into_iter()
        .map(|chunk| chunk.revision_id)
        .collect();
    assert_eq!(chunked, eligible);
    let manifest = manifest_of(&database, &chunk_set_of(&database, &scopes, &report));
    assert_eq!(manifest["revisions"], json!(eligible));
    for (path, _) in &documents[2..] {
        let revision = revision_of(&database, &scopes, path);
        assert_eq!(database.occurrences(&scopes, &revision).unwrap(), []);
    }
    // The four others are left out, each with why, in document order.
    let mut left_out = [
        ("reextract.md", Ineligibility::NeedsReextraction),
        ("quarantined.md", Ineligibility::Quarantined),
        ("excluded.md", Ineligibility::Excluded),
        ("undecided.md", Ineligibility::Undecided),
    ]
    .map(|(path, reason)| LeftOut {
        document: document_of(&database, &scopes, path),
        source_ref: format!("https://example.org/{path}"),
        revision: revision_of(&database, &scopes, path),
        reason,
    });
    left_out.sort_by(|first, second| first.document.cmp(&second.document));
    assert_eq!(report.left_out, 4);
    assert_eq!(report.left_out_documents, left_out);
    assert_eq!(
        manifest["left_out"],
        serde_json::to_value(&left_out).unwrap()
    );
    let reason = |path: &str| {
        let document = document_of(&database, &scopes, path);
        let entry = manifest["left_out"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["document"] == document.as_str())
            .unwrap()
            .clone();
        entry["reason"].clone()
    };
    assert_eq!(
        [
            "reextract.md",
            "quarantined.md",
            "excluded.md",
            "undecided.md"
        ]
        .map(reason),
        ["needs_reextraction", "quarantined", "excluded", "undecided"].map(|name| json!(name))
    );
}

#[test]
fn a_preparation_the_caller_cannot_read_whole_is_refused_before_anything_is_recorded() {
    let scratch = Scratch::new();
    scratch.corpus(&[("a.md", "# A\n\nSome words.\n")]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    decide(
        &database,
        &revision_of(&database, &scopes, "a.md"),
        Outcome::Accepted,
    );
    let (port, tokenizer) = tokenizer();
    let calls = port.texts().len();
    // A principal who reads the collection's one source, but not the
    // collection.
    let source = "workspace/default/collection/notes/source/docs"
        .parse()
        .unwrap();
    database
        .grant("partial", &source, Right::Read, "test")
        .unwrap();
    for scopes in [
        database.visible("partial").unwrap(),
        database.visible("nobody").unwrap(),
    ] {
        let Err(Error::NotVisible(scope)) = prepare(&database, &scopes, COLLECTION, &tokenizer)
        else {
            panic!("the preparation was not refused");
        };
        assert_eq!(scope, "workspace/default/collection/notes");
    }
    assert_eq!(port.texts().len(), calls, "nothing was counted");
    for table in ["chunk_sets", "chunks", "occurrences", "near_dup_groups"] {
        assert_eq!(scratch.rows(table), 0, "{table}");
    }
    assert_eq!(
        Error::NotVisible("workspace/default/collection/notes".to_owned()).to_string(),
        "the caller cannot read the scope `workspace/default/collection/notes`, whose \
         revisions the preparation chunks: grant it first"
    );
}

#[test]
fn a_revision_whose_canonical_document_is_another_revisions_is_refused_and_the_rest_prepared() {
    let scratch = Scratch::new();
    scratch.corpus(&[
        ("a.md", &format!("# A\n\n{}\n", words("alpha", 30))),
        ("b.md", &format!("# B\n\n{}\n", words("beta", 30))),
    ]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    // The revision `rev-mixed` of a document of its own, recorded with the
    // artifacts of the revision of `a.md`.
    let taken = database
        .revision(&scopes, &revision_of(&database, &scopes, "a.md"))
        .unwrap()
        .unwrap();
    let mixed = Document {
        id: "doc-mixed".to_owned(),
        collection_id: COLLECTION.to_owned(),
        source_id: "docs".to_owned(),
        source_ref: "https://example.org/mixed.md".to_owned(),
    };
    database.record_document(&mixed).unwrap();
    database
        .record_revision(&Revision {
            id: "rev-mixed".to_owned(),
            document_id: mixed.id.clone(),
            ..taken.clone()
        })
        .unwrap();
    decide_all(&database, &scopes, Outcome::Accepted);
    let (_, tokenizer) = tokenizer();
    let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    assert_eq!(
        [report.eligible, report.prepared, report.refused],
        [3, 2, 1]
    );
    assert_eq!(
        report.refusals,
        [Refusal {
            revision: "rev-mixed".to_owned(),
            document: "doc-mixed".to_owned(),
            source_ref: "https://example.org/mixed.md".to_owned(),
            reason: format!(
                "its canonical document is the revision {} of the document {}",
                taken.id, taken.document_id
            ),
        }]
    );
}
