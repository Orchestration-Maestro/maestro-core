//! A document's identity: its ID hashes its collection with its
//! `source_ref`, so one URL in two collections is two documents, and its
//! file is found relative to the manifest's own directory.

use super::support::{
    Scratch, declaration_of, everything, import, import_declared, line, markdown, page,
};
use maestro_kernel::{
    artifact::Digest,
    document::{Document, Revision},
    store::Database,
};

/// The documented recipe of a document's ID: `doc-` followed by the SHA-256
/// of `collection`, the collection's ID and the `source_ref`, each separated
/// by a NUL byte.
fn recipe(collection: &str, source_ref: &str) -> String {
    let named = format!("collection\0{collection}\0{source_ref}");
    format!("doc-{}", Digest::of(named.as_bytes()).as_str())
}

/// The one revision of the collection `collection`.
fn only_revision(database: &Database, collection: &str) -> Revision {
    let revisions = database
        .eligible_revisions(&everything(database), collection)
        .unwrap();
    let [revision] = revisions.as_slice() else {
        panic!("{revisions:#?}");
    };
    revision.clone()
}

#[test]
fn one_source_ref_in_two_collections_gives_two_documents() {
    let scratch = Scratch::new();
    let compost = markdown("compost");
    scratch.put("compost.md", &compost);
    scratch.manifest(&[line("compost.md", &compost, &page("compost"))]);
    let database = scratch.database();
    for collection in ["allotment", "orchard"] {
        let report = import(&scratch, &database, collection);
        assert_eq!(report.imported, 1, "{collection}");
    }
    let scopes = everything(&database);
    let [allotment, orchard] = ["allotment", "orchard"].map(|collection| {
        let revision = only_revision(&database, collection);
        assert_eq!(revision.document_id, recipe(collection, &page("compost")));
        database
            .document(&scopes, &revision.document_id)
            .unwrap()
            .unwrap()
    });
    assert_ne!(allotment.id, orchard.id);
    assert_eq!(
        allotment,
        Document {
            id: recipe("allotment", &page("compost")),
            collection_id: "allotment".to_owned(),
            source_id: "docs".to_owned(),
            source_ref: page("compost"),
        }
    );
}

#[test]
fn a_document_without_a_url_keeps_its_id_from_one_release_to_the_next() {
    let scratch = Scratch::new();
    let [first, second] = ["handover, 1.0", "handover, 2.0"].map(markdown);
    let source_ref = "corpus-path:runbooks/handover.md";
    let database = scratch.database();
    for (release, bytes) in [("1.0", &first), ("2.0", &second)] {
        let manifest = format!("{release}/manifest.jsonl");
        scratch.put(&format!("{release}/runbooks/handover.md"), bytes);
        scratch.manifest_at(
            &manifest,
            &[line("runbooks/handover.md", bytes, source_ref)],
        );
        let declaration = declaration_of("ops", &[("docs", &manifest)]);
        let report = import_declared(&scratch, &database, &declaration);
        assert_eq!(report.imported, 1, "{release}");
    }
    let revisions = database
        .eligible_revisions(&everything(&database), "ops")
        .unwrap();
    let ids: Vec<&str> = revisions
        .iter()
        .map(|revision| revision.document_id.as_str())
        .collect();
    assert_eq!(ids, [recipe("ops", source_ref), recipe("ops", source_ref)]);
}

#[test]
fn a_document_path_resolves_against_the_manifest_directory() {
    let scratch = Scratch::new();
    let [inside, beside] = ["compost, as the release gives it", "another compost"].map(markdown);
    scratch.put("releases/2026/pages/compost.md", &inside);
    scratch.put("pages/compost.md", &beside);
    scratch.manifest_at(
        "releases/2026/manifest.jsonl",
        &[line("pages/compost.md", &inside, &page("compost"))],
    );
    let database = scratch.database();
    let declaration = declaration_of("garden", &[("docs", "releases/2026/manifest.jsonl")]);
    let report = import_declared(&scratch, &database, &declaration);
    assert_eq!(
        [
            report.imported,
            report.unchanged,
            report.held,
            report.refused
        ],
        [1, 0, 0, 0]
    );
    let revision = only_revision(&database, "garden");
    assert_eq!(revision.original_digest, Digest::of(&inside));
    assert_eq!(database.get(&revision.original_digest).unwrap(), inside);
}
