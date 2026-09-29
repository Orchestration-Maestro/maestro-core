//! The stages of the production publication, search and evidence assembly,
//! traced on the synthetic corpus with fake inference: the spans they open,
//! their nesting and outcomes, and no corpus or question text in any field
//! they record (MR-04).
#![cfg(test)]

use maestro_knowledge::search::evidence::EvidenceSettings;

use super::{
    pipeline::published::{PRINCIPAL, Published, publish_fake},
    span_recorder::Recording,
    support::TestDirectory,
};
use maestro_kernel::{
    artifact::{Digest, Store},
    evidence::{Bundle, RequestBudget},
    gateway::{CardFields, Limits, ModelCard, Role, RouterEntry},
};
use maestro_knowledge::{
    index::Qdrant,
    search::{
        EvidenceInput, Reranker, SearchConfiguration, SearchContext, SearchError, SearchRequest,
        evidence::{EvidenceCounter, assemble_evidence},
        routes::dense::Embedder,
        search,
    },
};
use serde_json::Value;
use std::{
    fs,
    net::TcpListener,
    num::NonZeroU32,
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
    thread,
};
use tokio::runtime::{Builder, Runtime};

/// The stages of one publication, in the order they open, below the
/// batches it embeds.
const PUBLISH: [(&str, Option<&str>); 6] = [
    ("knowledge.publish", None),
    ("knowledge.publish.load_inputs", Some("knowledge.publish")),
    ("knowledge.publish.project", Some("knowledge.publish")),
    ("knowledge.publish.verify", Some("knowledge.publish")),
    ("knowledge.publish.switch_alias", Some("knowledge.publish")),
    ("knowledge.publish.kernel", Some("knowledge.publish")),
];

/// The stages of one search of the suite's first answerable question, a
/// global one, whose structured route refuses its form as unsupported, each
/// with its outcome when every route can run.
const SEARCH: [(&str, Option<&str>, &str); 7] = [
    ("retrieval.search", None, "ok"),
    ("retrieval.route.dense", Some("retrieval.search"), "ok"),
    ("retrieval.route.lexical", Some("retrieval.search"), "ok"),
    ("retrieval.route.identifier", Some("retrieval.search"), "ok"),
    (
        "retrieval.route.structured",
        Some("retrieval.search"),
        "unavailable",
    ),
    ("retrieval.fuse", Some("retrieval.search"), "ok"),
    ("retrieval.rerank", Some("retrieval.search"), "ok"),
];

/// The stages of one evidence assembly.
const ASSEMBLE: [(&str, Option<&str>); 7] = [
    ("retrieval.assemble", None),
    ("retrieval.assemble.ledger", Some("retrieval.assemble")),
    ("retrieval.assemble.candidates", Some("retrieval.assemble")),
    ("retrieval.assemble.sources", Some("retrieval.assemble")),
    ("retrieval.assemble.sections", Some("retrieval.assemble")),
    ("retrieval.assemble.selection", Some("retrieval.assemble")),
    ("retrieval.assemble.output", Some("retrieval.assemble")),
];

/// The shortest corpus line the content scan looks for: shorter lines, such
/// as a fence or a list marker, are not text a field could leak.
const SHORTEST_CANARY: usize = 12;

/// One recorded run of the synthetic path: the publication, a search of an
/// answerable question and its evidence assembly, then a refused search and
/// a search against a dead Qdrant, all on one published generation.
struct Traced {
    /// The publication, search and assembly.
    path: Recording,
    /// The refused and the degraded search.
    degraded: Recording,
    /// The bundle the assembly built.
    bundle: Bundle,
}

/// The synthetic generation is built once, the slow part, and every contract
/// is checked on it: each test process builds its own, so one test builds it
/// once instead of once for each contract. Every contract runs even when an
/// earlier one fails, and the test names each one that failed.
#[test]
fn the_synthetic_path_traces_its_stages_and_outcomes_and_no_content() {
    let traced = traced_synthetic_path();
    let contracts: [(&str, &dyn Fn()); 3] = [
        ("each_stage_is_nested_with_its_outcome", &|| {
            each_stage_is_nested_with_its_outcome(&traced.path, &traced.bundle);
        }),
        (
            "a_refused_search_and_unavailable_routes_record_their_outcomes",
            &|| {
                a_refused_search_and_unavailable_routes_record_their_outcomes(&traced.degraded);
            },
        ),
        (
            "no_recorded_field_carries_the_corpus_or_question_text",
            &|| {
                no_recorded_field_carries_the_corpus_or_question_text(&traced);
            },
        ),
    ];

    let failed: Vec<&str> = contracts
        .into_iter()
        .filter(|(_, contract)| panic::catch_unwind(AssertUnwindSafe(contract)).is_err())
        .map(|(name, _)| name)
        .collect();

    assert!(failed.is_empty(), "failed contracts: {failed:?}");
}

/// The publication, search and assembly open each stage under its parent,
/// with its outcome, duration and counts.
fn each_stage_is_nested_with_its_outcome(recording: &Recording, bundle: &Bundle) {
    let embeds = recording.named("knowledge.publish.embed");
    let stages = recording.stages();
    let expected: Vec<_> = PUBLISH[..3]
        .iter()
        .copied()
        .chain(
            embeds
                .iter()
                .map(|_| ("knowledge.publish.embed", Some("knowledge.publish.project"))),
        )
        .chain(PUBLISH[3..].iter().copied())
        .map(|(name, parent)| (name, parent, Some("ok")))
        .chain(SEARCH.map(|(name, parent, outcome)| (name, parent, Some(outcome))))
        .chain(ASSEMBLE.map(|(name, parent)| (name, parent, Some("ok"))))
        .collect();
    assert_eq!(stages, expected);

    let [publish] = recording.named("knowledge.publish")[..] else {
        panic!("one publication");
    };
    assert_eq!(publish.field("collection_id"), Some("synthetic"));
    assert_eq!(publish.field("generation"), Some("1"));
    let points: u64 = publish.field("points").unwrap().parse().unwrap();
    let embedded: u64 = embeds
        .iter()
        .map(|embed| embed.field("chunks").unwrap().parse::<u64>().unwrap())
        .sum();
    assert!(!embeds.is_empty());
    assert_eq!(embedded, points);
    let [assemble] = recording.named("retrieval.assemble")[..] else {
        panic!("one assembly");
    };
    assert_eq!(
        assemble.field("passages"),
        Some(bundle.passages.len().to_string().as_str())
    );
    let [search] = recording.named("retrieval.search")[..] else {
        panic!("one search");
    };
    assert_ne!(search.field("candidates"), Some("0"));
    for span in &recording.spans {
        if span.field("outcome").is_some() {
            assert!(span.field("duration_us").is_some(), "{span:?}");
        }
    }
}

/// A search of an unknown collection is refused; one against a dead Qdrant
/// runs with its dense and lexical routes and its rerank unavailable.
fn a_refused_search_and_unavailable_routes_record_their_outcomes(recording: &Recording) {
    let expected: Vec<_> = [("retrieval.search", None, Some("refused"))]
        .into_iter()
        .chain(SEARCH.map(|(name, parent, outcome)| {
            let outcome = match name {
                "retrieval.route.dense" | "retrieval.route.lexical" | "retrieval.rerank" => {
                    "unavailable"
                }
                _ => outcome,
            };
            (name, parent, Some(outcome))
        }))
        .collect();
    assert_eq!(recording.stages(), expected);
}

/// No field of either recording, on a span or an event, carries the text of
/// the corpus, its manifest or its questions.
fn no_recorded_field_carries_the_corpus_or_question_text(traced: &Traced) {
    let canaries = canaries(&fixture_root());

    let passage = &traced.bundle.passages[0].text;
    assert!(
        canaries
            .iter()
            .any(|canary| passage.contains(canary.as_str())),
        "the canaries must match the text a bundle carries"
    );
    assert!(
        traced.path.stages().len() > 10,
        "the run records its stages"
    );
    let fields = traced.path.fields().chain(traced.degraded.fields());
    for (name, value) in fields {
        for canary in &canaries {
            assert!(
                !value.contains(canary.as_str()) && !name.contains(canary.as_str()),
                "field {name} carries corpus or question text"
            );
        }
    }
}

/// Publishes the synthetic collection, searches its first answerable
/// question and assembles its evidence, recorded; then searches an unknown
/// collection, and the collection through a dead Qdrant, recorded apart.
fn traced_synthetic_path() -> Traced {
    let runtime = runtime();
    let scratch = TestDirectory::new().unwrap();
    let card = reranker(&scratch.path);
    let mut run = None;
    let path = Recording::of(|| {
        let published = publish_fake(&runtime).unwrap();
        let question = published
            .suite
            .questions
            .iter()
            .find(|question| question.answerable)
            .unwrap()
            .question
            .clone();
        let input = runtime
            .block_on(traced_search(
                &published,
                (&published.qdrant, &card),
                published.collection,
                &question,
            ))
            .unwrap();
        let assembled = runtime.block_on(assemble_evidence(
            published.database.clone(),
            input,
            EvidenceCounter::Utf8Bytes,
        ));
        run = Some((published, question, assembled.unwrap()));
    });
    let (published, question, bundle) = run.unwrap();
    assert!(!bundle.passages.is_empty());
    let dead = dead_qdrant();
    let degraded = Recording::of(|| {
        let refused = runtime.block_on(traced_search(
            &published,
            (&published.qdrant, &card),
            "absent",
            &question,
        ));
        assert!(matches!(refused, Err(SearchError::Admission(_))));
        let degraded = runtime.block_on(traced_search(
            &published,
            (&dead, &card),
            published.collection,
            &question,
        ));
        assert!(degraded.is_ok());
    });
    Traced {
        path,
        degraded,
        bundle,
    }
}

/// A Qdrant endpoint that closes every connection it accepts, so that its
/// routes fail at once on every host: Windows retries a connection to a
/// port nothing listens on for about two seconds, past the routes' window.
fn dead_qdrant() -> Qdrant {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || listener.incoming().for_each(drop));
    Qdrant::new(&format!("http://127.0.0.1:{port}")).unwrap()
}

/// Searches `collection` of `published` for `text` through `qdrant`, with
/// the fake embedder and the reranker `card`.
async fn traced_search(
    published: &Published,
    (qdrant, card): (&Qdrant, &ModelCard),
    collection: &str,
    text: &str,
) -> Result<EvidenceInput, SearchError> {
    let context = SearchContext {
        intent_expander: None,
        database: published.database.clone(),
        principal: PRINCIPAL,
        qdrant,
        embedder: Some(Embedder {
            port: &published.models,
            card: &published.card,
        }),
        reranker: Some(Reranker {
            port: &published.models,
            card,
        }),
        source_classes: None,
    };
    let request = SearchRequest {
        evidence: EvidenceSettings::default(),
        collection,
        text,
        version: None,
        budget: RequestBudget {
            deadline_ms: 10_000,
            ..RequestBudget::default()
        },
        configuration: SearchConfiguration::default(),
    };
    Box::pin(search(&context, &request)).await
}

/// A reranker card for the fake inference, recorded under `directory`.
fn reranker(directory: &Path) -> ModelCard {
    let fields = CardFields {
        role: Role::Reranker,
        router_entry: RouterEntry::parse("synthetic-reranker").unwrap(),
        file_digest: Digest::of(b"FakeModels synthetic reranker/1"),
        template_digest: None,
        server_build: "fake-models/1".to_owned(),
        dimensions: None,
        limits: Limits {
            context_tokens: NonZeroU32::new(8192).unwrap(),
            output_tokens: None,
        },
        suite_results: Vec::new(),
    };
    ModelCard::record(&Store::new(directory.join("artifacts")), &fields).unwrap()
}

/// A single-threaded runtime, as the synthetic gate runs on.
fn runtime() -> Runtime {
    Builder::new_current_thread().enable_all().build().unwrap()
}

/// The synthetic fixture's directory.
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("tests/fixtures/synthetic")
}

/// The text no field may carry: the fixture's own path; each document's
/// path, source reference and title from the manifest; every line of every
/// document long enough to be text; and every question of the suite.
fn canaries(root: &Path) -> Vec<String> {
    let corpus = root.join("corpus");
    let mut canaries = vec![root.to_string_lossy().into_owned()];
    let manifest = fs::read_to_string(corpus.join("maestro-corpus.jsonl")).unwrap();
    for entry in manifest.lines() {
        let entry: Value = serde_json::from_str(entry).unwrap();
        for key in ["path", "source_ref", "title"] {
            canaries.push(entry[key].as_str().unwrap().to_owned());
        }
        let document = fs::read_to_string(corpus.join(entry["path"].as_str().unwrap())).unwrap();
        canaries.extend(
            document
                .lines()
                .map(str::trim)
                .filter(|line| line.len() >= SHORTEST_CANARY)
                .map(str::to_owned),
        );
    }
    let suite = fs::read_to_string(root.join("evals/synthetic.jsonl")).unwrap();
    for question in suite.lines() {
        let question: Value = serde_json::from_str(question).unwrap();
        canaries.push(question["question"].as_str().unwrap().to_owned());
    }
    canaries
}
