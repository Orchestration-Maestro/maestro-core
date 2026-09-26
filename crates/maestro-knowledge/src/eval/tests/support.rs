//! What the evaluation tests share: questions, bundles built from the hits a
//! test lists, with their trace and their routes' statuses, the report of a
//! run's results, and a comparison of computed numbers with hand-computed
//! ones.

use crate::{
    eval::{self, QuestionResult, Report, metric::measure},
    suite::{Language, Question, Schema},
};
use maestro_kernel::{
    artifact::Digest,
    evidence::{self, Budget, Bundle, Passage, RouteStatus, Span, Trace},
};
use std::collections::BTreeMap;

/// The collection the tests' bundles search.
pub(super) const COLLECTION: &str = "synthetic";
/// The generation the tests' bundles are pinned to.
pub(super) const GENERATION: i64 = 7;

/// The question `id`, answerable or not. The runner judges a question by the
/// section IDs its names resolve to, so its own names are left out.
pub(super) fn question(id: &str, answerable: bool) -> Question {
    Question {
        schema: Schema::V1,
        id: id.to_owned(),
        language: Language::En,
        question: format!("What does {id} ask?"),
        answerable,
        expected: Vec::new(),
    }
}

/// A passage of a bundle as a test lists it: its number, the section it
/// belongs to, the reranker's score and the routes that found it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Hit {
    /// The passage's number.
    pub(super) n: u32,
    /// The section its text belongs to.
    pub(super) section: Option<&'static str>,
    /// The reranker's score, absent when the reranker did not run.
    pub(super) score: Option<f64>,
    /// The routes that found it.
    pub(super) routes: &'static [&'static str],
}

/// The passage `n` of `section`, scored `score` and found by `bm25` and
/// `dense`.
pub(super) fn hit(n: u32, section: &'static str, score: f64) -> Hit {
    Hit {
        n,
        section: Some(section),
        score: Some(score),
        routes: &["bm25", "dense"],
    }
}

/// The passage `n` of `section`, unscored, as when the reranker did not run.
pub(super) fn unscored(n: u32, section: &'static str) -> Hit {
    Hit {
        score: None,
        ..hit(n, section, 0.0)
    }
}

/// The routes of a search where every route and the reranker ran.
pub(super) const ALL_RAN: [(&str, Option<&str>); 3] =
    [("bm25", None), ("dense", None), ("rerank", None)];

/// The bundle of `hits`, listed in their order, where every route and the
/// reranker ran.
pub(super) fn bundle(hits: &[Hit]) -> Bundle {
    bundle_with(hits, &ALL_RAN)
}

/// The bundle of `hits`, listed in their order, with `routes`: each name
/// with the reason it could not run, or none when it ran.
pub(super) fn bundle_with(hits: &[Hit], routes: &[(&str, Option<&str>)]) -> Bundle {
    Bundle {
        schema: evidence::Schema::V1,
        collection: COLLECTION.to_owned(),
        generation: GENERATION,
        query: "What does it ask?".to_owned(),
        lang: "en".to_owned(),
        routes: routes
            .iter()
            .map(|(name, reason)| {
                let status = reason.map_or(RouteStatus::Ok, |reason| {
                    RouteStatus::Unavailable(reason.to_owned())
                });
                ((*name).to_owned(), status)
            })
            .collect(),
        passages: hits.iter().map(passage).collect(),
        conflicts: Vec::new(),
        known_gaps: Vec::new(),
        budget: Budget {
            evidence_tokens: 0,
            limit: 6000,
        },
        trace: hits
            .iter()
            .map(|hit| Trace {
                n: hit.n,
                score: hit.score,
                routes: hit.routes.iter().map(|&route| route.to_owned()).collect(),
                procedural: false,
            })
            .collect(),
    }
}

/// The passage `hit` lists, whose text names its number.
fn passage(hit: &Hit) -> Passage {
    let text = format!("The text of passage {}.", hit.n);
    Passage {
        n: hit.n,
        section_id: hit.section.map(str::to_owned),
        document_id: "document".to_owned(),
        revision_id: "revision".to_owned(),
        title: "A document".to_owned(),
        section_path: vec!["A document".to_owned()],
        version: None,
        source_ref: "https://handbook.example.org/document".to_owned(),
        span: Span {
            start: 0,
            end: text.len(),
        },
        digest: Digest::of(text.as_bytes()),
        text,
        alternates: Vec::new(),
    }
}

/// The report of a run of the suite `synthetic` over [`GENERATION`] of
/// [`COLLECTION`] whose questions got `questions`, with no profile, its
/// intervals drawn with the seed 1.
pub(super) fn report_of(questions: Vec<QuestionResult>) -> Report {
    Report {
        schema: eval::Schema::V1,
        suite: "synthetic".to_owned(),
        collection: COLLECTION.to_owned(),
        generation: GENERATION,
        profiles: BTreeMap::new(),
        seed: 1,
        degraded_searches: questions.iter().filter(|result| result.degraded).count(),
        metrics: measure(&questions, 1),
        questions,
    }
}

/// `expected`, as the section IDs a question expects.
pub(super) fn sections(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|&section| section.to_owned()).collect()
}

/// Asserts that `computed` is `expected`, a hand-computed number, to the
/// last few bits.
#[track_caller]
pub(super) fn close(computed: f64, expected: f64) {
    assert!(
        (computed - expected).abs() <= 1e-12,
        "computed {computed}, expected {expected}"
    );
}
