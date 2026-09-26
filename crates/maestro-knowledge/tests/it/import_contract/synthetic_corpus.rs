//! The public synthetic collection, `tests/fixtures/synthetic` (T014),
//! imported end to end as its declaration names it: each line of its
//! manifest becomes the eligible revision of a document of its own, the two
//! identical glossaries share their original artifact, and a second import
//! finds everything unchanged.

use super::support::{Scratch, data_of, events};
use maestro_kernel::{artifact::Digest, binding::Bindings, scope::Right};
use maestro_knowledge::{collection::Declaration, import};
use serde_json::json;
use std::{collections::BTreeSet, fs, path::Path};

/// The digest of the glossary that two lines of the manifest give, each
/// from its own `source_ref`.
const GLOSSARY: &str = "ec4fe2fe069b3896e7a4cb12da66ae926aa93e81c4fe0a141d62a7f78390ed62";

#[test]
fn the_synthetic_collection_imports_end_to_end() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("tests")
        .join("fixtures")
        .join("synthetic");
    let declaration: Declaration = fs::read_to_string(fixture.join("collection.json"))
        .unwrap()
        .parse()
        .unwrap();
    let bindings: Bindings = format!("synthetic_root = '{}'\n", fixture.display())
        .parse()
        .unwrap();
    let scratch = Scratch::new();
    let database = scratch.database();
    let collection = "workspace/default/collection/synthetic".parse().unwrap();
    database
        .grant("reader", &collection, Right::Read, "test")
        .unwrap();
    let scopes = database.visible("reader").unwrap();
    let first = import::import(&database, &scopes, &declaration, &bindings).unwrap();
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        json!({
            "collection": "synthetic", "imported": 28, "unchanged": 0, "held": 0, "refused": 0,
            "refusals": [],
        })
    );
    let revisions = database.eligible_revisions(&scopes, "synthetic").unwrap();
    assert_eq!(revisions.len(), 28, "no document failed canonicalization");
    let documents: BTreeSet<&str> = revisions
        .iter()
        .map(|revision| revision.document_id.as_str())
        .collect();
    assert_eq!(documents.len(), 28, "one document per source_ref");
    let glossary = Digest::parse(GLOSSARY).unwrap();
    assert_eq!(database.artifact(&glossary).unwrap().unwrap().pins, 2);
    let second = import::import(&database, &scopes, &declaration, &bindings).unwrap();
    assert_eq!(
        [
            second.imported,
            second.unchanged,
            second.held,
            second.refused
        ],
        [0, 28, 0, 0]
    );
    assert_eq!(
        data_of(
            &events(&database, "synthetic"),
            "maestro.knowledge.import.completed.v1"
        ),
        [(28, 0), (0, 28)].map(|(imported, unchanged)| json!({
            "collection": "synthetic",
            "imported": imported,
            "unchanged": unchanged,
            "held": 0,
            "refused": 0,
        }))
    );
}
