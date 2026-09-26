//! The public synthetic collection, `tests/fixtures/synthetic` (T014),
//! prepared end to end: its two identical glossaries prepared once, its two
//! on-call handovers and its two certificate renewals each grouped as near
//! duplicates, and every chunk resolving to the source text it covers.

use super::scratch::{Scratch, chunk_set_of, chunks_of, decide, everything, tokenizer};
use crate::{collection::Declaration, import, prepare::prepare};
use maestro_kernel::{
    binding::Bindings, chunk_set::ChunkSetState, document::Outcome, scope::ScopeSet,
    store::Database,
};
use std::{collections::BTreeSet, fs, path::Path, slice};

/// The `source_ref` of every document of `revisions`, in order.
fn sources(database: &Database, scopes: &ScopeSet, revisions: &[String]) -> BTreeSet<String> {
    revisions
        .iter()
        .map(|id| {
            let revision = database.revision(scopes, id).unwrap().unwrap();
            let document = database.document(scopes, &revision.document_id).unwrap();
            document.unwrap().source_ref
        })
        .collect()
}

#[test]
fn the_synthetic_collection_prepares_end_to_end() {
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
    let scopes = everything(&database);
    import::import(&database, &scopes, &declaration, &bindings).unwrap();
    let synthetic = "synthetic";
    for revision in database.eligible_revisions(&scopes, synthetic).unwrap() {
        decide(&database, &revision.id, Outcome::Accepted);
    }
    let (_, tokenizer) = tokenizer();
    let report = prepare(&database, &scopes, synthetic, &tokenizer).unwrap();
    assert_eq!(
        [
            report.eligible,
            report.prepared,
            report.duplicates,
            report.near_duplicate_groups,
            report.refused
        ],
        [28, 27, 1, 2, 0]
    );
    assert_eq!(
        chunk_set_of(&database, &scopes, &report).state,
        ChunkSetState::Complete
    );
    // The glossaries: prepared once, occurring twice.
    let glossaries: Vec<String> = database
        .eligible_revisions(&scopes, synthetic)
        .unwrap()
        .into_iter()
        .filter(|revision| {
            sources(&database, &scopes, slice::from_ref(&revision.id))
                .iter()
                .any(|source| source.ends_with("glossary") || source.ends_with("glossary.md"))
        })
        .map(|revision| revision.id)
        .collect();
    let chunked = glossaries.iter().min().unwrap();
    let places: BTreeSet<String> = database
        .occurrences(&scopes, chunked)
        .unwrap()
        .into_iter()
        .map(|occurrence| occurrence.source_ref)
        .collect();
    assert_eq!(
        places,
        BTreeSet::from([
            "corpus-path:en/operations/glossary.md".to_owned(),
            "https://handbook.example.org/4.2/glossary".to_owned(),
        ])
    );
    // The near duplicates: the handovers, and the renewals of two releases.
    let mut groups = BTreeSet::new();
    let chunks = chunks_of(&database, &scopes, &report);
    for chunk in &chunks {
        let group = database
            .near_duplicates(&scopes, &chunk.revision_id)
            .unwrap();
        assert!(group.iter().all(|member| member.jaccard >= 0.85));
        let members: Vec<String> = group.into_iter().map(|member| member.revision_id).collect();
        if !members.is_empty() {
            groups.insert(sources(&database, &scopes, &members));
        }
    }
    assert_eq!(
        groups,
        BTreeSet::from([
            BTreeSet::from([
                "corpus-path:en/operations/on-call-handover.md".to_owned(),
                "corpus-path:en/operations/payments/on-call-handover.md".to_owned(),
            ]),
            BTreeSet::from([
                "https://handbook.example.org/4.1/tls/certificate-renewal".to_owned(),
                "https://handbook.example.org/4.2/tls/certificate-renewal".to_owned(),
            ]),
        ])
    );
    // Every chunk resolves to the source text it covers.
    assert_eq!(u64::try_from(chunks.len()).unwrap(), report.chunks);
    for chunk in &chunks {
        assert!(chunk.token_count <= 700);
        let excerpt = database
            .resolve(&scopes, &report.chunk_set, &chunk.id)
            .unwrap();
        assert!(!excerpt.text.trim().is_empty(), "{}", chunk.id);
    }
}
