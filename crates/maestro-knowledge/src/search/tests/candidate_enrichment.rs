//! Opt-in reranker enrichment degrades to the indexed chunk, never to a failed search.

use super::support::CandidateDb;
use crate::search::{CandidateContext, SectionClassSet, SectionPrior, candidates};
use std::slice;

fn all_classes() -> SectionClassSet {
    let mut classes = SectionClassSet::default();
    for name in ["changelog", "release_notes", "conversion"] {
        assert!(classes.insert(name));
    }
    classes
}

#[tokio::test]
async fn an_unreadable_source_keeps_the_chunk_text_and_records_the_fallback() {
    let db = CandidateDb::new(b"prepared text", "docs");
    let mut request = db.request(&db.chunk_id, vec![db.revision_id.clone()]);
    request.configuration.candidate_context = CandidateContext::BoundedSection { max_bytes: 1500 };
    request.configuration.section_prior = SectionPrior::Soft {
        weight: 0.5,
        classes: all_classes(),
    };
    let loaded = candidates::load(db.database.clone(), request)
        .await
        .unwrap();
    assert_eq!(loaded.candidates.len(), 1);
    assert_eq!(loaded.candidates[0].text, "prepared text");
    assert_eq!(loaded.fallbacks, slice::from_ref(&db.chunk_id));
    assert!(loaded.penalized.is_empty());
    assert_eq!(loaded.context_unavailable, 1);
}

#[tokio::test]
async fn an_unreadable_source_never_fails_a_prior_only_search() {
    let db = CandidateDb::new(b"prepared text", "docs");
    let mut request = db.request(&db.chunk_id, vec![db.revision_id.clone()]);
    request.configuration.section_prior = SectionPrior::Soft {
        weight: 0.5,
        classes: all_classes(),
    };
    let loaded = candidates::load(db.database.clone(), request)
        .await
        .unwrap();
    assert_eq!(loaded.candidates[0].text, "prepared text");
    assert!(loaded.fallbacks.is_empty());
    assert!(loaded.penalized.is_empty());
    assert_eq!(loaded.context_unavailable, 1);
}

#[tokio::test]
async fn a_prior_without_classes_reads_no_source() {
    let db = CandidateDb::new(b"prepared text", "docs");
    let mut request = db.request(&db.chunk_id, vec![db.revision_id.clone()]);
    request.configuration.section_prior = SectionPrior::Soft {
        weight: 0.5,
        classes: SectionClassSet::default(),
    };
    let loaded = candidates::load(db.database.clone(), request)
        .await
        .unwrap();
    assert_eq!(loaded.candidates[0].text, "prepared text");
    assert_eq!(loaded.context_unavailable, 0);
}
