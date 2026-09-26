//! `run` resolves each expected section of a suite in the canonical document
//! the caller's lookup gives, looking each document up once, then retrieves
//! and judges every question in the suite's order; an empty heading path
//! expects a document without sections whole. It refuses a name that gives no
//! one section, or no document without sections, two names of one section or
//! of one document, and a bundle of another collection or generation, and
//! passes the caller's own errors through.

use super::support::{COLLECTION, GENERATION, Hit, bundle, close, hit, whole};
use crate::{
    eval::{Expected, Header, Report, RunError, Schema, metric::measure, run},
    suite::{Question, Suite, Unresolved},
};
use maestro_canonicalization::{CanonicalDocument, CanonicalizeInput, canonicalize};
use maestro_kernel::evidence::{Bundle, RouteStatus};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    convert::Infallible,
    error, io,
    num::NonZeroU32,
    time::{Duration, Instant},
};

/// A document of two sections under its title.
const BACKUPS: &str = "https://handbook.example.org/backups";
/// A document whose heading path `Queues › Retries` repeats.
const QUEUES: &str = "corpus-path:queues.md";
/// A document without a heading, so without a section.
const NOTES: &str = "corpus-path:notes.md";
/// Another document without a section.
const LOGS: &str = "corpus-path:logs.md";

/// The canonical document of `source_ref`, if the tests hold one.
fn document(source_ref: &str) -> Option<CanonicalDocument> {
    let markdown = match source_ref {
        BACKUPS => {
            "# Backups\n\nWhat backups keep.\n\n## Retention\n\nThirty days.\n\n\
             ## Restore\n\nFrom the latest copy.\n"
        }
        QUEUES => {
            "# Queues\n\n## Retries\n\nThree times.\n\n## Retries\n\nThen the dead letters.\n"
        }
        NOTES => "Retries stop after three.\n\n**Cause**\n\nThe queue is full.\n",
        LOGS => "Logs rotate every day.\n\n**Cause**\n\nThe disk fills up.\n",
        _ => return None,
    };
    let mut input = CanonicalizeInput::new(markdown, "document.md");
    input.metadata.source_reference = Some(source_ref.to_owned());
    Some(canonicalize(input).unwrap())
}

/// The ID of the `occurrence`th section of `source_ref` under `path`.
fn section_id(source_ref: &str, path: &[&str], occurrence: usize) -> &'static str {
    let document = document(source_ref).unwrap();
    let section = document
        .sections
        .iter()
        .filter(|section| section.heading_path == path)
        .nth(occurrence - 1)
        .unwrap();
    section.section_id.clone().leak()
}

/// The ID of the document of `source_ref`.
fn document_id(source_ref: &str) -> &'static str {
    document(source_ref).unwrap().document_id.leak()
}

/// The suite line of the question `id`, answerable when it expects sections.
fn line(id: &str, expected: &Value) -> String {
    json!({
        "schema": "maestro-suite/1", "id": id, "language": "en",
        "question": format!("What about {id}?"),
        "answerable": expected != &json!([]), "expected": expected,
    })
    .to_string()
}

/// The suite of `lines`.
fn suite(lines: &[String]) -> Suite {
    lines.join("\n").parse().unwrap()
}

/// A suite of three questions: one found first, one expecting the second of
/// two sections with one heading path, one unanswerable.
fn three() -> Suite {
    suite(&[
        line(
            "retention",
            &json!([{"source_ref": BACKUPS, "heading_path": ["Backups", "Retention"]}]),
        ),
        line(
            "second-retry",
            &json!([{
                "source_ref": QUEUES, "heading_path": ["Queues", "Retries"], "occurrence": 2,
            }]),
        ),
        line("unknown", &json!([])),
    ])
}

/// What the runs of these tests evaluate.
fn header() -> Header {
    Header {
        suite: "small".to_owned(),
        collection: COLLECTION.to_owned(),
        generation: GENERATION,
        profiles: BTreeMap::from([("sparse".to_owned(), "bm25-en-fr/1".to_owned())]),
        seed: 5,
    }
}

/// The bundles of the three questions: `retention` first, `second-retry`
/// second, and `unknown` left empty.
fn answer(question: &Question) -> Bundle {
    let hits: Vec<Hit> = match question.id.as_str() {
        "retention" => vec![hit(
            1,
            section_id(BACKUPS, &["Backups", "Retention"], 1),
            0.9,
        )],
        "second-retry" => vec![
            hit(1, section_id(QUEUES, &["Queues", "Retries"], 1), 0.9),
            hit(2, section_id(QUEUES, &["Queues", "Retries"], 2), 0.5),
        ],
        _ => Vec::new(),
    };
    bundle(&hits)
}

/// The retrieval of the three questions, [`answer`], which never fails.
fn answers() -> impl FnMut(&Question) -> Result<Bundle, Infallible> {
    |question| Ok(answer(question))
}

/// The lookup of the tests' documents, which never fails.
fn lookup() -> impl FnMut(&str) -> Result<Option<CanonicalDocument>, Infallible> {
    |source_ref| Ok(document(source_ref))
}

/// The refusal of a run of `suite` whose retrieval must never be reached.
fn refusal(suite: &Suite) -> RunError<Infallible> {
    run(header(), suite, lookup(), |question| {
        panic!("{} retrieved before the suite was resolved", question.id)
    })
    .unwrap_err()
}

#[test]
fn a_run_judges_each_question_in_the_suites_order_under_its_header() {
    let report = run(header(), &three(), lookup(), answers()).unwrap();
    assert_eq!(report.schema, Schema::V1);
    assert_eq!(report.suite, "small");
    assert_eq!(report.suite_digest, three().digest);
    assert_eq!(report.collection, COLLECTION);
    assert_eq!(report.generation, GENERATION);
    assert_eq!(report.profiles, header().profiles);
    assert_eq!(report.seed, 5);
    let ids: Vec<&str> = report
        .questions
        .iter()
        .map(|result| result.id.as_str())
        .collect();
    assert_eq!(ids, ["retention", "second-retry", "unknown"]);
    let ranks: Vec<Vec<(Option<&str>, Option<u32>)>> = report
        .questions
        .iter()
        .map(|result| {
            let expected = result.expected.iter();
            expected
                .map(|expected| {
                    let rank = expected.rank.map(NonZeroU32::get);
                    (expected.section_id.as_deref(), rank)
                })
                .collect()
        })
        .collect();
    assert_eq!(
        ranks,
        [
            vec![(
                Some(section_id(BACKUPS, &["Backups", "Retention"], 1)),
                Some(1)
            )],
            vec![(Some(section_id(QUEUES, &["Queues", "Retries"], 2)), Some(2))],
            vec![],
        ]
    );
    let documents: Vec<&str> = report.questions[..2]
        .iter()
        .map(|result| result.expected[0].document_id.as_str())
        .collect();
    assert_eq!(documents, [document_id(BACKUPS), document_id(QUEUES)]);
    assert_eq!(report.metrics, measure(&report.questions, 5));
    close(report.metrics.mrr_at_10.unwrap().value, 0.75);
}

#[test]
fn each_document_is_looked_up_once() {
    let looked_up = RefCell::new(Vec::new());
    let suite = suite(&[
        line(
            "retention",
            &json!([{"source_ref": BACKUPS, "heading_path": ["Backups", "Retention"]}]),
        ),
        line(
            "restore",
            &json!([{"source_ref": BACKUPS, "heading_path": ["Backups", "Restore"]}]),
        ),
        line(
            "both",
            &json!([
                {"source_ref": QUEUES, "heading_path": ["Queues", "Retries"], "occurrence": 1},
                {"source_ref": BACKUPS, "heading_path": ["Backups"]},
            ]),
        ),
    ]);
    let lookup = |source_ref: &str| {
        looked_up.borrow_mut().push(source_ref.to_owned());
        Ok::<_, Infallible>(document(source_ref))
    };
    run(header(), &suite, lookup, |_| Ok(bundle(&[]))).unwrap();
    let mut looked_up = looked_up.into_inner();
    looked_up.sort();
    assert_eq!(looked_up, [QUEUES, BACKUPS].map(str::to_owned));
}

#[test]
fn a_document_without_sections_is_expected_whole_and_held_by_its_passages() {
    let suite = suite(&[line(
        "whole",
        &json!([
            {"source_ref": NOTES, "heading_path": []},
            {"source_ref": BACKUPS, "heading_path": ["Backups", "Restore"]},
        ]),
    )]);
    let notes = document_id(NOTES);
    let retrieve = |_: &Question| {
        let hits = [hit(1, "unrelated", 0.9), whole(2, notes, 0.5)];
        Ok::<_, Infallible>(bundle(&hits))
    };
    let report = run(header(), &suite, lookup(), retrieve).unwrap();
    let whole = Expected {
        document_id: notes.to_owned(),
        section_id: None,
        rank: NonZeroU32::new(2),
    };
    let restore = Expected {
        document_id: document_id(BACKUPS).to_owned(),
        section_id: Some(section_id(BACKUPS, &["Backups", "Restore"], 1).to_owned()),
        rank: None,
    };
    assert_eq!(report.questions[0].expected, [whole, restore]);
}

#[test]
fn two_documents_without_sections_expected_by_one_question_are_each_ranked() {
    let suite = suite(&[line(
        "two-whole",
        &json!([
            {"source_ref": NOTES, "heading_path": []},
            {"source_ref": LOGS, "heading_path": []},
        ]),
    )]);
    let (notes, logs) = (document_id(NOTES), document_id(LOGS));
    assert_ne!(notes, logs);
    let retrieve = |_: &Question| {
        let hits = [whole(1, logs, 0.9), whole(2, notes, 0.5)];
        Ok::<_, Infallible>(bundle(&hits))
    };
    let report = run(header(), &suite, lookup(), retrieve).unwrap();
    let whole = |document_id: &str, rank| Expected {
        document_id: document_id.to_owned(),
        section_id: None,
        rank: NonZeroU32::new(rank),
    };
    assert_eq!(
        report.questions[0].expected,
        [whole(notes, 2), whole(logs, 1)]
    );
}

#[test]
fn an_empty_heading_path_in_a_document_with_sections_refuses_the_run() {
    let error = refusal(&suite(&[line(
        "sectioned",
        &json!([{"source_ref": BACKUPS, "heading_path": []}]),
    )]));
    assert!(matches!(
        &error,
        RunError::Unresolved {
            question,
            source_ref,
            heading_path,
            reason: Unresolved::HasSections { sections: 3 },
        } if question == "sectioned" && source_ref == BACKUPS && heading_path.is_empty()
    ));
    assert_eq!(
        error.to_string(),
        format!(
            "question sectioned expects the whole of {BACKUPS}: an empty heading path names a \
             document without sections, but this document has 3 sections"
        )
    );
}

#[test]
fn a_document_the_lookup_does_not_find_refuses_the_run() {
    let missing = "https://handbook.example.org/missing";
    let error = refusal(&suite(&[line(
        "lost",
        &json!([{"source_ref": missing, "heading_path": ["Missing"]}]),
    )]));
    assert!(
        matches!(&error, RunError::NoDocument { question, source_ref }
            if question == "lost" && source_ref == missing)
    );
    assert_eq!(
        error.to_string(),
        format!("question lost expects {missing}, which the generation does not hold")
    );
}

#[test]
fn a_name_that_gives_no_one_section_refuses_the_run() {
    let error = refusal(&suite(&[line(
        "nowhere",
        &json!([{"source_ref": BACKUPS, "heading_path": ["Backups", "Nothing"]}]),
    )]));
    assert!(matches!(
        &error,
        RunError::Unresolved { question, source_ref, heading_path, reason: Unresolved::NoSection }
            if question == "nowhere" && source_ref == BACKUPS
                && heading_path == &["Backups", "Nothing"]
    ));
    assert_eq!(
        error.to_string(),
        format!(
            "question nowhere expects Backups › Nothing in {BACKUPS}, which names no one section: \
             no section has this heading path"
        )
    );
    let cause = error::Error::source(&error).unwrap();
    assert_eq!(cause.to_string(), "no section has this heading path");
    let error = refusal(&suite(&[line(
        "which",
        &json!([{"source_ref": QUEUES, "heading_path": ["Queues", "Retries"]}]),
    )]));
    assert!(matches!(
        error,
        RunError::Unresolved {
            reason: Unresolved::Ambiguous { sections: 2 },
            ..
        }
    ));
}

#[test]
fn two_names_of_one_section_refuse_the_run() {
    let retention = json!({"source_ref": BACKUPS, "heading_path": ["Backups", "Retention"]});
    let error = refusal(&suite(&[line("twice", &json!([retention, retention]))]));
    let id = section_id(BACKUPS, &["Backups", "Retention"], 1);
    assert!(
        matches!(&error, RunError::SameSection { question, section_id }
            if question == "twice" && section_id == id)
    );
    assert_eq!(
        error.to_string(),
        format!("question twice names the section {id} twice")
    );
    assert!(error::Error::source(&error).is_none());
}

#[test]
fn two_names_of_one_document_refuse_the_run() {
    let notes = json!({"source_ref": NOTES, "heading_path": []});
    let error = refusal(&suite(&[line("twice", &json!([notes, notes]))]));
    let id = document_id(NOTES);
    assert!(
        matches!(&error, RunError::SameDocument { question, document_id }
            if question == "twice" && document_id == id)
    );
    assert_eq!(
        error.to_string(),
        format!("question twice names the document {id} twice")
    );
    assert!(error::Error::source(&error).is_none());
}

#[test]
fn a_bundle_of_another_collection_or_generation_refuses_the_run() {
    let later = |_: &Question| {
        let mut bundle = bundle(&[]);
        bundle.generation = GENERATION + 1;
        Ok::<_, Infallible>(bundle)
    };
    let error = run(header(), &three(), lookup(), later).unwrap_err();
    assert!(
        matches!(&error, RunError::OtherGeneration { question, collection, generation }
            if question == "retention" && collection == COLLECTION && *generation == GENERATION + 1)
    );
    assert_eq!(
        error.to_string(),
        "question retention was answered from generation 8 of synthetic, \
         not from the generation the run evaluates"
    );
    let other = |_: &Question| {
        let mut bundle = bundle(&[]);
        bundle.collection = "ctm".to_owned();
        Ok::<_, Infallible>(bundle)
    };
    let error = run(header(), &three(), lookup(), other).unwrap_err();
    assert!(matches!(&error, RunError::OtherGeneration { collection, .. } if collection == "ctm"));
}

#[test]
fn the_callers_own_errors_pass_through() {
    let locked = |_: &str| Err(io::Error::other("the database is locked"));
    let error = run(header(), &three(), locked, |_| Ok(bundle(&[]))).unwrap_err();
    // Documents are looked up in the order of their source_ref.
    assert!(matches!(&error, RunError::Documents { source_ref, .. } if source_ref == QUEUES));
    assert_eq!(
        error.to_string(),
        format!("the canonical document of {QUEUES} could not be read: the database is locked")
    );
    let cause = error::Error::source(&error).unwrap();
    assert_eq!(cause.to_string(), "the database is locked");
    let down = |_: &Question| Err(io::Error::other("the search is down"));
    let lookup = |source_ref: &str| Ok(document(source_ref));
    let error = run(header(), &three(), lookup, down).unwrap_err();
    assert!(matches!(&error, RunError::Retrieval { question, .. } if question == "retention"));
    assert_eq!(
        error.to_string(),
        "question retention could not be retrieved: the search is down"
    );
    let cause = error::Error::source(&error).unwrap();
    assert_eq!(cause.to_string(), "the search is down");
}

#[test]
fn each_retrieval_is_timed() {
    let slow = |question: &Question| {
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(2) {}
        Ok(answer(question))
    };
    let report: Report = run(header(), &three(), lookup(), slow).unwrap();
    for result in &report.questions {
        assert!(
            result.latency_us >= 2000,
            "{}: {}",
            result.id,
            result.latency_us
        );
    }
}

#[test]
fn a_run_counts_its_degraded_searches() {
    let without_reranker = |question: &Question| {
        let mut bundle = answer(question);
        if question.id == "second-retry" {
            let reason = "no room for the reranker".to_owned();
            bundle
                .routes
                .insert("rerank".to_owned(), RouteStatus::Unavailable(reason));
        }
        Ok::<_, Infallible>(bundle)
    };
    let report = run(header(), &three(), lookup(), without_reranker).unwrap();
    let degraded: Vec<bool> = report
        .questions
        .iter()
        .map(|result| result.degraded)
        .collect();
    assert_eq!(degraded, [false, true, false]);
    assert_eq!(report.degraded_searches, 1);
}
