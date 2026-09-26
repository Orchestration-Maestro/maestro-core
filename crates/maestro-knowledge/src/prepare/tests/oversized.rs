//! A unit that cannot fit 700 tokens with its context refuses its document,
//! naming the unit: the refusal is recorded with the chunk set, the document
//! gets no chunk, and the rest are prepared.

use super::scratch::{
    COLLECTION, Scratch, chunk_set_of, chunks_of, decide_all, manifest_of, revision_of, tokenizer,
    words,
};
use crate::prepare::{Refusal, prepare};
use maestro_canonicalization::{BlockType, CanonicalDocument};
use maestro_kernel::{chunk_set::ChunkSetState, document::Outcome};
use serde_json::json;

#[test]
fn a_unit_over_700_tokens_refuses_its_document_by_name_and_the_rest_are_prepared() {
    let scratch = Scratch::new();
    // A heading of 800 words, 800 tokens for the port, leaves no room for
    // the paragraph below it, which carries it as context.
    let oversized = format!("# {}\n\ntail\n", words("heading", 800));
    let small = format!("# Small\n\n{}\n", words("delta", 30));
    scratch.corpus(&[("oversized.md", &oversized), ("small.md", &small)]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    decide_all(&database, &scopes, Outcome::Accepted);
    let (_, tokenizer) = tokenizer();
    let report = prepare(&database, &scopes, COLLECTION, &tokenizer).unwrap();
    let refused = revision_of(&database, &scopes, "oversized.md");
    let revision = database.revision(&scopes, &refused).unwrap().unwrap();
    let canonical: CanonicalDocument =
        serde_json::from_slice(&database.get(&revision.canonical_digest).unwrap()).unwrap();
    let tail = canonical
        .blocks
        .iter()
        .find(|block| block.block_type == BlockType::Paragraph)
        .unwrap();
    let span = tail.source_spans[0];
    let named = format!(
        " of block {} at bytes [{}, {}) does not fit in 700 tokens with its context",
        tail.block_id, span.start, span.end
    );
    assert_eq!(
        [report.eligible, report.prepared, report.refused],
        [2, 1, 1]
    );
    let [refusal] = report.refusals.as_slice() else {
        panic!("{:?}", report.refusals);
    };
    assert!(
        refusal.reason.starts_with("the unit unit-"),
        "{}",
        refusal.reason
    );
    assert!(refusal.reason.ends_with(&named), "{}", refusal.reason);
    assert_eq!(
        refusal,
        &Refusal {
            revision: refused.clone(),
            document: revision.document_id.clone(),
            source_ref: "https://example.org/oversized.md".to_owned(),
            reason: refusal.reason.clone(),
        }
    );
    let chunks = chunks_of(&database, &scopes, &report);
    assert!(!chunks.is_empty());
    assert!(chunks.iter().all(|chunk| chunk.revision_id != refused));
    let set = chunk_set_of(&database, &scopes, &report);
    assert_eq!(set.state, ChunkSetState::Complete);
    assert_eq!(
        manifest_of(&database, &set)["refusals"],
        json!([{
            "revision": refused,
            "document": revision.document_id,
            "source_ref": "https://example.org/oversized.md",
            "reason": refusal.reason,
        }])
    );
}
