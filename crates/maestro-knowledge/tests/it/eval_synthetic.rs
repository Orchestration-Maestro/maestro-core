//! The evaluation runner over the public synthetic suite (T014), end to end
//! with fake retrievals: an oracle that returns exactly the expected sections
//! scores every metric at its best, a blind retrieval fails every question,
//! and the comparison of the two gains the whole scale, with certainty. A
//! report names the digest of its suite's file, and is recorded in a kernel
//! under its own collection, generation and suite.
#![cfg(test)]

use maestro_canonicalization::{CanonicalDocument, CanonicalizeInput, canonicalize};
use maestro_kernel::{
    artifact::Digest,
    chunk_set::NewChunkSet,
    document::Collection,
    eval::Error as KernelError,
    evidence::{Budget, Bundle, Passage, RouteStatus, Schema, Span, Trace},
    generation::NewGeneration,
    store::Database,
};
use maestro_knowledge::{
    collection::Declaration,
    corpus::Entry,
    eval::{self, Estimate, FailureClass, Header, Report},
    suite::{Question, Resolved, Suite},
};
use std::{
    collections::BTreeMap,
    convert::Infallible,
    env, error, fs,
    num::NonZeroU32,
    path::{Path, PathBuf},
    process, str,
};

/// The generation the fake retrievals answer from.
const GENERATION: i64 = 3;

/// The fixture directory of the synthetic collection.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("tests")
        .join("fixtures")
        .join("synthetic")
}

/// The declaration of the synthetic collection.
fn declaration() -> Declaration {
    fs::read_to_string(root().join("collection.json"))
        .unwrap()
        .parse()
        .unwrap()
}

/// The file of the suite `synthetic`, where the declaration names it.
fn suite_file() -> PathBuf {
    declaration()
        .evals
        .suite
        .under(&root())
        .join("synthetic.jsonl")
}

/// The suite `synthetic`, and the canonical document of each line of the
/// collection's corpus manifest, by `source_ref`, as its declaration names
/// them under the fixture directory.
fn synthetic() -> (Suite, Documents) {
    let (root, declaration) = (root(), declaration());
    let suite = fs::read_to_string(suite_file()).unwrap().parse().unwrap();
    let manifest = declaration.sources[0].manifest.path.under(&root);
    let corpus = manifest.parent().unwrap();
    let documents = fs::read_to_string(&manifest)
        .unwrap()
        .lines()
        .map(|line| {
            let entry: Entry = line.parse().unwrap();
            let markdown = fs::read_to_string(entry.path.under(corpus)).unwrap();
            let mut input = CanonicalizeInput::new(&markdown, entry.path.as_str());
            input.metadata.source_reference = Some(entry.source_ref.clone());
            input.metadata.title = Some(entry.title.clone());
            (entry.source_ref, canonicalize(input).unwrap())
        })
        .collect();
    (suite, documents)
}

/// What the runs evaluate: the suite `synthetic` over generation 3.
fn header() -> Header {
    Header {
        suite: "synthetic".to_owned(),
        collection: "synthetic".to_owned(),
        generation: GENERATION,
        profiles: BTreeMap::from([("chunking".to_owned(), "structural-500-700/1".to_owned())]),
        seed: 42,
    }
}

/// A bundle of one passage for each of `sections`, in that rank, found by
/// `bm25` and `dense` while both ran.
fn bundle(sections: &[Option<String>]) -> Bundle {
    let numbered = (1..).zip(sections);
    Bundle {
        schema: Schema::V1,
        collection: "synthetic".to_owned(),
        generation: GENERATION,
        query: String::new(),
        lang: "en".to_owned(),
        routes: BTreeMap::from([
            ("bm25".to_owned(), RouteStatus::Ok),
            ("dense".to_owned(), RouteStatus::Ok),
        ]),
        passages: numbered
            .clone()
            .map(|(n, section)| {
                let text = format!("Passage {n}.");
                Passage {
                    n,
                    section_id: section.clone(),
                    document_id: "document".to_owned(),
                    revision_id: "revision".to_owned(),
                    title: "Handbook".to_owned(),
                    section_path: Vec::new(),
                    version: None,
                    source_ref: "https://handbook.example.org/".to_owned(),
                    span: Span {
                        start: 0,
                        end: text.len(),
                    },
                    digest: Digest::of(text.as_bytes()),
                    text,
                    alternates: Vec::new(),
                }
            })
            .collect(),
        conflicts: Vec::new(),
        known_gaps: Vec::new(),
        budget: Budget {
            evidence_tokens: 0,
            limit: 6000,
        },
        trace: numbered
            .map(|(n, _)| Trace {
                n,
                score: Some(1.0 / f64::from(n)),
                routes: vec!["bm25".to_owned(), "dense".to_owned()],
                procedural: false,
            })
            .collect(),
    }
}

/// The certain estimate of `value`.
fn certain(value: f64) -> Estimate {
    Estimate {
        value,
        low: value,
        high: value,
    }
}

/// The canonical documents of the synthetic collection, by `source_ref`.
type Documents = BTreeMap<String, CanonicalDocument>;

/// The report of a run of the synthetic suite, retrieved by `retrieve`, which
/// may read the collection's documents.
fn evaluate(retrieve: impl Fn(&Question, &Documents) -> Bundle) -> Report {
    let (suite, documents) = synthetic();
    let lookup = |source_ref: &str| Ok::<_, Infallible>(documents.get(source_ref).cloned());
    let answer = |question: &Question| Ok(retrieve(question, &documents));
    let report = eval::run(header(), &suite, lookup, answer).unwrap();
    let text = serde_json::to_string(&report).unwrap();
    assert_eq!(text.parse::<Report>().unwrap(), report);
    report
}

/// The oracle: each expected section in the suite's order, then a passage of
/// no section.
fn oracle(question: &Question, documents: &Documents) -> Bundle {
    let mut sections: Vec<Option<String>> = question
        .expected
        .iter()
        .map(|expected| {
            let document = &documents[&expected.source_ref];
            let Ok(Resolved::Section(section)) = expected.resolve(document) else {
                panic!("the synthetic suite expects sections only: {expected:?}");
            };
            Some(section.section_id.clone())
        })
        .collect();
    if !sections.is_empty() {
        sections.push(None);
    }
    bundle(&sections)
}

#[test]
fn an_oracle_retrieval_scores_every_metric_at_its_best() {
    let report = evaluate(oracle);
    assert_eq!(report.questions.len(), 56);
    for result in &report.questions {
        let ranks: Vec<Option<u32>> = result
            .expected
            .iter()
            .map(|expected| expected.rank.map(NonZeroU32::get))
            .collect();
        let first: Vec<Option<u32>> = (1..).map(Some).take(ranks.len()).collect();
        assert_eq!(ranks, first, "{}", result.id);
        assert_eq!(result.failures, [], "{}", result.id);
    }
    let metrics = report.metrics;
    assert_eq!(metrics.recall_at_5, Some(certain(1.0)));
    assert_eq!(metrics.recall_at_10, Some(certain(1.0)));
    assert_eq!(metrics.mrr_at_10, Some(certain(1.0)));
    assert_eq!(metrics.ndcg_at_10, Some(certain(1.0)));
    assert_eq!(metrics.no_answer_accuracy, Some(certain(1.0)));
    assert_eq!(metrics.false_abstentions, Some(certain(0.0)));
}

#[test]
fn a_blind_retrieval_fails_every_question_and_the_comparison_gains_the_whole_scale() {
    let blind = evaluate(|_, _| bundle(&[Some("unrelated".to_owned())]));
    let answerable = blind.questions.iter().filter(|result| result.answerable);
    assert_eq!(answerable.clone().count(), 48);
    for result in answerable {
        let classes: Vec<(u32, FailureClass)> = result
            .failures
            .iter()
            .map(|failure| (failure.cutoff, failure.class))
            .collect();
        assert_eq!(
            classes,
            [
                (5, FailureClass::NotRetrieved),
                (10, FailureClass::NotRetrieved)
            ],
            "{}",
            result.id
        );
        for failure in &result.failures {
            assert_eq!(failure.missed_by, ["bm25", "dense"]);
        }
    }
    assert_eq!(blind.metrics.recall_at_10, Some(certain(0.0)));
    assert_eq!(blind.metrics.no_answer_accuracy, Some(certain(0.0)));
    let comparison = eval::compare(&blind, &evaluate(oracle), 7).unwrap();
    assert_eq!(comparison.questions, 56);
    assert_eq!(comparison.differences.recall_at_10, Some(certain(1.0)));
    assert_eq!(comparison.differences.mrr_at_10, Some(certain(1.0)));
    assert_eq!(
        comparison.differences.no_answer_accuracy,
        Some(certain(1.0))
    );
}

#[test]
fn a_report_names_the_digest_of_its_suite_file() {
    let file = fs::read(suite_file()).unwrap();
    assert_eq!(evaluate(oracle).suite_digest, Digest::of(&file));
}

/// A new empty directory under the platform's temporary directory, removed
/// with everything in it when dropped: after the database a test opened in
/// it, which it declares later.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = env::temp_dir().join(format!("maestro-knowledge-eval-{name}-{}", process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// A kernel in this directory with the collection `synthetic` and
    /// `generations` generations of it, numbered from 1, each built from
    /// one complete chunk set.
    fn kernel(&self, generations: i64) -> Database {
        let database = Database::open_in(&self.0).unwrap();
        database
            .record_collection(&Collection {
                id: "synthetic".to_owned(),
                title: "The synthetic collection".to_owned(),
                visibility: "public".to_owned(),
                profiles: BTreeMap::new(),
            })
            .unwrap();
        let set = NewChunkSet {
            id: "synthetic-set",
            collection_id: "synthetic",
            chunk_profile: "mapped-structural-chunks/2",
            counter_contract_id: "router/1:test",
        };
        database.begin_chunk_set(&set).unwrap();
        let manifest = database.put(b"{}", "application/json").unwrap();
        database.complete_chunk_set(set.id, &manifest).unwrap();
        for _ in 0..generations {
            database
                .create_generation(&NewGeneration {
                    collection_id: "synthetic".to_owned(),
                    chunk_set_id: set.id.to_owned(),
                    embedding_profile: "embed:test".to_owned(),
                    sparse_profile: "bm25-en-fr/1".to_owned(),
                })
                .unwrap();
        }
        database
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn a_report_is_recorded_under_its_own_collection_generation_and_suite() {
    let report = evaluate(oracle);
    let scratch = Scratch::new("recorded");
    let database = scratch.kernel(GENERATION);
    let recorded = eval::record(&database, &report).unwrap();
    assert_eq!(recorded.collection_id, "synthetic");
    assert_eq!(recorded.generation, GENERATION);
    assert_eq!(recorded.suite, "synthetic");
    let json = database.get(&recorded.digest).unwrap();
    assert_eq!(
        str::from_utf8(&json).unwrap().parse::<Report>().unwrap(),
        report
    );
}

#[test]
fn a_report_of_a_generation_the_kernel_lacks_is_refused_naming_it() {
    let report = evaluate(oracle);
    let scratch = Scratch::new("refused");
    let database = scratch.kernel(GENERATION - 1);
    let error = eval::record(&database, &report).unwrap_err();
    assert!(
        matches!(
            &error,
            eval::RecordError::Kernel(KernelError::UnknownGeneration { collection, generation })
                if collection == "synthetic" && *generation == GENERATION
        ),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "the report could not be recorded: the collection synthetic records no generation 3"
    );
    let source = error::Error::source(&error).map(ToString::to_string);
    assert_eq!(
        source.as_deref(),
        Some("the collection synthetic records no generation 3")
    );
}
