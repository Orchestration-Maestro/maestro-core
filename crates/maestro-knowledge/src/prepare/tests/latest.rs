//! A document is prepared by its latest revision alone, in record order, as
//! the quality gate lets it through (T020's `quality::eligible`): an older
//! revision never stands in for a newer one, and a document whose latest
//! revision is held back, failed or not decided yet is left out, and said so
//! in the report and the manifest.

use super::scratch::{
    COLLECTION, Scratch, chunk_set_of, chunks_of, decide, decide_all, document_of, manifest_of,
    revision_of, revisions_of, tokenizer, words,
};
use crate::prepare::{Ineligibility, LeftOut, Report, prepare};
use maestro_kernel::{document::Outcome, scope::ScopeSet, store::Database};
use serde_json::json;
use std::collections::BTreeSet;

/// A page titled `title`, of 40 words of `stem`.
fn page(title: &str, stem: &str) -> String {
    format!("# {title}\n\n{}\n", words(stem, 40))
}

/// A page whose front matter contradicts the title its manifest line gives,
/// `Notes`: canonicalization fails.
const FAILED: &str = "---\ntitle: Another page\n---\n# Page\n\n\
    What the page says about the backups of the database.\n";

/// The revisions of the chunks of the chunk set `report` names.
fn chunked(database: &Database, scopes: &ScopeSet, report: &Report) -> BTreeSet<String> {
    chunks_of(database, scopes, report)
        .into_iter()
        .map(|chunk| chunk.revision_id)
        .collect()
}

#[test]
fn a_document_is_prepared_by_its_latest_revision_alone() {
    let scratch = Scratch::new();
    let other = page("Other", "other");
    scratch.corpus(&[("page.md", &page("Page", "first")), ("other.md", &other)]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    decide_all(&database, &scopes, Outcome::Accepted);
    // Rewritten at the same source_ref: a second revision of the one page,
    // accepted as the first was.
    scratch.corpus(&[("page.md", &page("Page", "second")), ("other.md", &other)]);
    scratch.import(&database);
    decide_all(&database, &scopes, Outcome::Accepted);
    let [first, latest]: [String; 2] = revisions_of(&database, &scopes, "page.md")
        .try_into()
        .unwrap();
    let unchanged = revision_of(&database, &scopes, "other.md");
    let (_, tokenizer) = tokenizer();
    let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    assert_eq!(
        [report.eligible, report.prepared, report.left_out],
        [2, 2, 0]
    );
    let expected = BTreeSet::from([latest, unchanged]);
    assert_eq!(chunked(&database, &scopes, &report), expected);
    assert_eq!(database.occurrences(&scopes, &first).unwrap(), []);
    let manifest = manifest_of(&database, &chunk_set_of(&database, &scopes, &report));
    assert_eq!(manifest["revisions"], json!(expected));
    assert_eq!(manifest["left_out"], json!([]));
}

#[test]
fn a_document_whose_latest_revision_is_held_or_failed_is_left_out_and_reported() {
    let scratch = Scratch::new();
    let kept = page("Kept", "kept");
    scratch.corpus(&[
        ("held.md", &page("Held", "held")),
        ("failed.md", &page("Failed", "failed")),
        ("kept.md", &kept),
    ]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    decide_all(&database, &scopes, Outcome::Accepted);
    // Both pages rewritten: one into a page the gate holds back, the other
    // into one whose canonicalization fails.
    scratch.corpus(&[
        ("held.md", &page("Held", "rewritten")),
        ("failed.md", FAILED),
        ("kept.md", &kept),
    ]);
    scratch.import(&database);
    let [held, failed] = ["held.md", "failed.md"].map(|path| revision_of(&database, &scopes, path));
    decide(&database, &held, Outcome::Quarantined);
    // Accepted by hand all the same: a failed revision is never eligible.
    decide(&database, &failed, Outcome::Accepted);
    let (_, tokenizer) = tokenizer();
    let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    assert_eq!(
        [report.eligible, report.prepared, report.left_out],
        [1, 1, 2]
    );
    assert_eq!(
        chunked(&database, &scopes, &report),
        BTreeSet::from([revision_of(&database, &scopes, "kept.md")]),
        "no older revision stands in for a latest one held or failed"
    );
    let mut left_out = [
        ("held.md", held, Ineligibility::Quarantined),
        ("failed.md", failed, Ineligibility::Failed),
    ]
    .map(|(path, revision, reason)| LeftOut {
        document: document_of(&database, &scopes, path),
        source_ref: format!("https://example.org/{path}"),
        revision,
        reason,
    });
    left_out.sort_by(|first, second| first.document.cmp(&second.document));
    assert_eq!(report.left_out_documents, left_out);
    let manifest = manifest_of(&database, &chunk_set_of(&database, &scopes, &report));
    assert_eq!(
        manifest["left_out"],
        serde_json::to_value(&left_out).unwrap()
    );
    let reasons: BTreeSet<String> = manifest["left_out"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["reason"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        reasons,
        BTreeSet::from(["failed".to_owned(), "quarantined".to_owned()])
    );
}
