//! Opt-in reranker enrichment degrades to the indexed chunk, never to a failed search.

use super::{clock::control, support::CandidateDb};
use crate::search::{
    CandidateContext, IntentExpansion, SearchConfiguration, SectionClassSet, SectionPrior,
    candidate_enrichment::{self, Settings},
    candidates,
};
use maestro_test_clock::on_stopped_clock;
use std::future;
use std::{collections::BTreeSet, num::NonZeroUsize, slice, time::Duration};

#[test]
fn elapsed_microseconds_preserve_the_measured_value() {
    assert_eq!(candidate_enrichment::micros(Duration::from_micros(42)), 42);
}

fn all_classes() -> SectionClassSet {
    let mut classes = SectionClassSet::default();
    for name in ["changelog", "release_notes", "conversion"] {
        assert!(classes.insert(name));
    }
    classes
}

#[test]
fn enrichment_loads_only_the_revisions_it_scores() {
    let db = CandidateDb::new(b"prepared text", "docs");
    let chunks = db
        .database
        .chunks(&db.scopes, &db.generation.chunk_set_id)
        .unwrap();
    let mut extra = chunks[0].clone();
    extra.id = "extra-candidate".to_owned();
    extra.revision_id = "extra-revision".to_owned();
    let mut candidates = vec![
        (&chunks[0], "prepared text".to_owned()),
        (&extra, "unscored text".to_owned()),
    ];
    let configuration = SearchConfiguration {
        rerank_depth: NonZeroUsize::new(1).unwrap(),
        intent_expansion: IntentExpansion::Hyde,
        intent_rerank_additions: 1,
        section_prior: SectionPrior::Soft {
            weight: 0.5,
            classes: all_classes(),
        },
        ..SearchConfiguration::default()
    };
    let control = control();
    let deadline = control.deadline;
    let enriched = candidate_enrichment::enrich(
        (&db.database, &db.scopes, &control),
        &Settings {
            configuration,
            query: "generic question",
            generation: &db.generation,
            deadline,
        },
        &mut candidates,
    );

    assert_eq!(
        enriched.requested_revisions,
        BTreeSet::from([db.revision_id.clone()])
    );
}

#[tokio::test]
async fn an_unreadable_source_keeps_the_chunk_text_and_records_the_fallback() {
    let db = CandidateDb::new(b"prepared text", "docs");
    let loaded = on_stopped_clock(future::pending(), || async {
        let mut request = db.request(&db.chunk_id, vec![db.revision_id.clone()]);
        request.configuration.candidate_context =
            CandidateContext::BoundedSection { max_bytes: 1500 };
        request.configuration.section_prior = SectionPrior::Soft {
            weight: 0.5,
            classes: all_classes(),
        };
        candidates::load(db.database.clone(), request).await
    })
    .await
    .unwrap();
    assert_eq!(loaded.candidates.len(), 1);
    assert_eq!(loaded.candidates[0].text, "prepared text");
    assert_eq!(loaded.fallbacks, slice::from_ref(&db.chunk_id));
    assert!(loaded.penalized.section.is_empty());
    assert_eq!(loaded.context_unavailable, 1);
}

#[tokio::test]
async fn an_unreadable_source_never_fails_a_prior_only_search() {
    let db = CandidateDb::new(b"prepared text", "docs");
    let loaded = on_stopped_clock(future::pending(), || async {
        let mut request = db.request(&db.chunk_id, vec![db.revision_id.clone()]);
        request.configuration.section_prior = SectionPrior::Soft {
            weight: 0.5,
            classes: all_classes(),
        };
        candidates::load(db.database.clone(), request).await
    })
    .await
    .unwrap();
    assert_eq!(loaded.candidates[0].text, "prepared text");
    assert!(loaded.fallbacks.is_empty());
    assert!(loaded.penalized.section.is_empty());
    assert_eq!(loaded.context_unavailable, 1);
}

#[tokio::test]
async fn a_prior_without_classes_reads_no_source() {
    let db = CandidateDb::new(b"prepared text", "docs");
    let loaded = on_stopped_clock(future::pending(), || async {
        let mut request = db.request(&db.chunk_id, vec![db.revision_id.clone()]);
        request.configuration.section_prior = SectionPrior::Soft {
            weight: 0.5,
            classes: SectionClassSet::default(),
        };
        candidates::load(db.database.clone(), request).await
    })
    .await
    .unwrap();
    assert_eq!(loaded.candidates[0].text, "prepared text");
    assert_eq!(loaded.context_unavailable, 0);
}
