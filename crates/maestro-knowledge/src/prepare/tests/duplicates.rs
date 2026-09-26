//! Duplicates (01 §6): exact duplicates are prepared once and keep every
//! occurrence; near duplicates are grouped with their confirmed Jaccard, and
//! each is still prepared.

use super::scratch::{
    COLLECTION, Scratch, chunk_set_of, chunks_of, decide_all, manifest_of, revision_of, tokenizer,
    words,
};
use crate::prepare::prepare;
use maestro_canonicalization::{
    CanonicalDocument, DedupInput, DedupScope, RevisionKey, WarningPolicy, group_exact,
};
use maestro_kernel::{
    document::{NearDuplicate, Occurrence, Outcome},
    scope::ScopeSet,
    store::Database,
};
use serde_json::json;
use std::collections::BTreeSet;

/// Where the content of `revision` occurs: the document `path` of the
/// collection's source `docs`.
fn occurrence(revision: &str, path: &str) -> Occurrence {
    Occurrence {
        revision_id: revision.to_owned(),
        collection_id: COLLECTION.to_owned(),
        source_id: "docs".to_owned(),
        source_ref: format!("https://example.org/{path}"),
    }
}

#[test]
fn exact_duplicates_are_prepared_once_as_the_smallest_revision_and_keep_every_occurrence() {
    let scratch = Scratch::new();
    let original = format!("# Topic\n\n{}\n", words("alpha", 60));
    let other = format!("# Other\n\n{}\n", words("beta", 60));
    scratch.titled_corpus(&[
        ("a.md", "Notes", &original),
        ("copy.md", "Notes", &original),
        ("b.md", "Notes", &other),
        // The same bytes under another title are other canonical content.
        ("retitled.md", "Other notes", &original),
    ]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    decide_all(&database, &scopes, Outcome::Accepted);
    let (_, tokenizer) = tokenizer();
    let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    assert_eq!(
        [
            report.eligible,
            report.prepared,
            report.duplicates,
            report.refused
        ],
        [4, 3, 1, 0]
    );
    let [first, copy, distinct, retitled] = ["a.md", "copy.md", "b.md", "retitled.md"]
        .map(|path| revision_of(&database, &scopes, path));
    let (chunked, duplicate) = if first < copy {
        (first, copy)
    } else {
        (copy, first)
    };
    let with_chunks: BTreeSet<String> = chunks_of(&database, &scopes, &report)
        .into_iter()
        .map(|chunk| chunk.revision_id)
        .collect();
    assert_eq!(
        with_chunks,
        BTreeSet::from([chunked.clone(), distinct.clone(), retitled.clone()])
    );
    assert_eq!(
        database.occurrences(&scopes, &chunked).unwrap(),
        [
            occurrence(&chunked, "a.md"),
            occurrence(&chunked, "copy.md")
        ]
    );
    assert_eq!(database.occurrences(&scopes, &duplicate).unwrap(), []);
    assert_eq!(
        database.occurrences(&scopes, &distinct).unwrap(),
        [occurrence(&distinct, "b.md")]
    );
    assert_eq!(
        database.occurrences(&scopes, &retitled).unwrap(),
        [occurrence(&retitled, "retitled.md")]
    );
    let manifest = manifest_of(&database, &chunk_set_of(&database, &scopes, &report));
    assert_eq!(manifest["duplicates"], json!({ duplicate: chunked }));
}

#[test]
fn near_duplicates_are_grouped_with_their_confirmed_jaccard_and_each_is_prepared() {
    let scratch = Scratch::new();
    let text = words("gamma", 200);
    // One word changed: 5 of the 197 word 5-grams differ.
    let near = text.replacen("gamma100 ", "changed ", 1);
    // Every twentieth word changed: 46 differ, a Jaccard of 151 / 243 with the first.
    let far: String = text
        .split(' ')
        .enumerate()
        .map(|(index, word)| if index % 20 == 19 { "changed" } else { word })
        .collect::<Vec<_>>()
        .join(" ");
    scratch.corpus(&[
        ("x.md", &format!("# Topic\n\n{text}\n")),
        ("y.md", &format!("# Topic\n\n{near}\n")),
        ("z.md", &format!("# Topic\n\n{far}\n")),
    ]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    decide_all(&database, &scopes, Outcome::Accepted);
    let (_, tokenizer) = tokenizer();
    let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    assert_eq!(
        [
            report.eligible,
            report.prepared,
            report.near_duplicate_groups
        ],
        [3, 3, 1]
    );
    let [x, y, z] = ["x.md", "y.md", "z.md"].map(|path| revision_of(&database, &scopes, path));
    let group = database.near_duplicates(&scopes, &x).unwrap();
    let jaccard = 192.0 / 202.0;
    let mut members = [x.clone(), y.clone()];
    members.sort();
    assert_eq!(
        group,
        members.map(|revision| NearDuplicate {
            group_id: group[0].group_id.clone(),
            revision_id: revision,
            jaccard,
        })
    );
    assert!(group[0].group_id.starts_with("near-"));
    assert_eq!(database.near_duplicates(&scopes, &z).unwrap(), []);
    let with_chunks: BTreeSet<String> = chunks_of(&database, &scopes, &report)
        .into_iter()
        .map(|chunk| chunk.revision_id)
        .collect();
    assert_eq!(
        with_chunks,
        BTreeSet::from([x, y, z]),
        "grouped, never deleted"
    );
    let manifest = manifest_of(&database, &chunk_set_of(&database, &scopes, &report));
    assert_eq!(
        manifest["near_duplicate_groups"],
        json!([group[0].group_id])
    );
}

/// The SHA-256 fingerprints canonicalization gives the revision `revision`
/// alone: of its original bytes, and of its canonical content.
fn fingerprints(database: &Database, scopes: &ScopeSet, revision: &str) -> (String, String) {
    let revision = database.revision(scopes, revision).unwrap().unwrap();
    let canonical: CanonicalDocument =
        serde_json::from_slice(&database.get(&revision.canonical_digest).unwrap()).unwrap();
    let markdown = String::from_utf8(database.get(&revision.original_digest).unwrap()).unwrap();
    let scope = DedupScope {
        tenant_id: "test".to_owned(),
        workspace_id: "test".to_owned(),
        authorized_revisions: BTreeSet::from([RevisionKey {
            document_id: canonical.document_id.clone(),
            revision_id: canonical.revision_id.clone(),
        }]),
    };
    let input = DedupInput {
        document: &canonical,
        markdown: &markdown,
    };
    let grouped = group_exact(&scope, &[input], WarningPolicy::Preserve).unwrap();
    let occurrence = &grouped.occurrences[0];
    (
        occurrence.original_hash.clone(),
        occurrence.canonical_hash.clone(),
    )
}

#[test]
fn the_same_canonical_content_from_other_bytes_is_no_exact_duplicate() {
    let scratch = Scratch::new();
    let original = format!("# Topic\n\n{}\n", words("alpha", 60));
    // A trailing blank line: other bytes, the same canonical content.
    let blank = format!("{original}\n");
    scratch.corpus(&[("a.md", &original), ("blank.md", &blank)]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    decide_all(&database, &scopes, Outcome::Accepted);
    let [first, second] = ["a.md", "blank.md"].map(|path| revision_of(&database, &scopes, path));
    let (first_hashes, second_hashes) = (
        fingerprints(&database, &scopes, &first),
        fingerprints(&database, &scopes, &second),
    );
    assert_eq!(
        first_hashes.1, second_hashes.1,
        "the same canonical content"
    );
    assert_ne!(first_hashes.0, second_hashes.0, "other original bytes");
    let (_, tokenizer) = tokenizer();
    let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    assert_eq!([report.prepared, report.duplicates], [2, 0]);
    for (revision, path) in [(&first, "a.md"), (&second, "blank.md")] {
        assert_eq!(
            database.occurrences(&scopes, revision).unwrap(),
            [occurrence(revision, path)]
        );
    }
}

#[test]
fn the_revision_prepared_for_exact_duplicates_does_not_depend_on_the_order_of_import() {
    let original = format!("# Topic\n\n{}\n", words("alpha", 60));
    let mut outcomes = Vec::new();
    for order in [["a.md", "copy.md"], ["copy.md", "a.md"]] {
        let scratch = Scratch::new();
        scratch.corpus(&order.map(|path| (path, original.as_str())));
        let database = scratch.database();
        let scopes = scratch.import(&database);
        decide_all(&database, &scopes, Outcome::Accepted);
        let (_, tokenizer) = tokenizer();
        let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
        let chunked: BTreeSet<String> = chunks_of(&database, &scopes, &report)
            .into_iter()
            .map(|chunk| chunk.revision_id)
            .collect();
        let [chunked]: [String; 1] = Vec::from_iter(chunked).try_into().unwrap();
        let places = database.occurrences(&scopes, &chunked).unwrap();
        let smallest = order
            .map(|path| revision_of(&database, &scopes, path))
            .into_iter()
            .min()
            .unwrap();
        assert_eq!(chunked, smallest);
        outcomes.push((chunked, places));
    }
    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(
        outcomes[0].1,
        [
            occurrence(&outcomes[0].0, "a.md"),
            occurrence(&outcomes[0].0, "copy.md")
        ]
    );
}
