//! Reports: `maestro-eval-report/1`, what a run measured, question by
//! question and metric by metric, read back as strictly as it is written.

use crate::shape;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, MapAccess, Visitor},
};
use std::{collections::BTreeMap, fmt, str::FromStr};

/// What a run evaluates, which its report names: the suite, the generation
/// of a collection with its profiles, and the seed of its intervals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// The suite, such as `synthetic`: `<name>.jsonl` in the directory the
    /// collection's `evals.suite` names.
    pub suite: String,
    /// The collection whose generation is evaluated.
    pub collection: String,
    /// The generation evaluated, which every bundle must be pinned to.
    pub generation: i64,
    /// The profiles the generation and its search use, by stage, such as
    /// `chunking` or `sparse`.
    pub profiles: BTreeMap<String, String>,
    /// The seed the intervals' resamples are drawn with.
    pub seed: u64,
}

/// A run's report, `maestro-eval-report/1`. [`str::parse`] reads one from a
/// JSON object only, strictly, and `serde_json` writes one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Report {
    /// The contract it follows.
    #[serde(deserialize_with = "shape::name")]
    pub schema: Schema,
    /// The suite run, as [`Header::suite`].
    pub suite: String,
    /// The collection, as [`Header::collection`].
    pub collection: String,
    /// The generation, as [`Header::generation`].
    pub generation: i64,
    /// The profiles, as [`Header::profiles`].
    #[serde(deserialize_with = "distinct_texts")]
    pub profiles: BTreeMap<String, String>,
    /// The seed, as [`Header::seed`].
    pub seed: u64,
    /// What each question got, in the suite's order.
    #[serde(deserialize_with = "shape::objects")]
    pub questions: Vec<QuestionResult>,
    /// How many of its questions' searches were degraded: a route or the
    /// reranker could not run. Their latencies are left out of the
    /// percentiles (SC-S1-004).
    pub degraded_searches: usize,
    /// The metrics over the questions, each with its interval.
    #[serde(deserialize_with = "shape::object")]
    pub metrics: Metrics,
}

impl FromStr for Report {
    type Err = serde_json::Error;

    /// The report `text` holds: one JSON object and nothing after it.
    fn from_str(text: &str) -> Result<Self, serde_json::Error> {
        shape::parse(text)
    }
}

/// The contract a report follows; this version writes and reads the first
/// only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Schema {
    /// `maestro-eval-report/1`.
    #[serde(rename = "maestro-eval-report/1")]
    V1,
}

/// What one question got: the rank of each section it expects, the size of
/// its bundle, how long its retrieval took, and its failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct QuestionResult {
    /// The question's id in its suite.
    pub id: String,
    /// Whether any section answers it.
    pub answerable: bool,
    /// The sections it expects, in the suite's order, each with its rank.
    #[serde(deserialize_with = "shape::objects")]
    pub expected: Vec<Expected>,
    /// How many passages its bundle held: none is an abstention.
    pub passages: usize,
    /// How long its retrieval took, in microseconds; one that took longer
    /// than `u32::MAX` of them, about 71 minutes, is written as that.
    pub latency_us: u32,
    /// Whether its search was degraded: a route or the reranker could not
    /// run. It counts in every retrieval metric, but its latency is left out
    /// of the percentiles.
    pub degraded: bool,
    /// Its failures, one for each cut-off at which no expected section
    /// ranks; an unanswerable question has none.
    #[serde(deserialize_with = "shape::objects")]
    pub failures: Vec<Failure>,
}

impl QuestionResult {
    /// The best rank of an expected section, if the bundle holds any.
    #[must_use]
    pub fn best_rank(&self) -> Option<u32> {
        self.expected
            .iter()
            .filter_map(|expected| expected.rank)
            .min()
    }
}

/// A section a question expects, and where its bundle ranked it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Expected {
    /// The section's ID in the evaluated generation.
    pub section_id: String,
    /// The rank of its best passage, from 1, absent when no passage holds
    /// it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<u32>,
}

/// A failure of an answerable question at a metric's cut-off: no expected
/// section ranks within it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Failure {
    /// The cut-off: 5 for Recall@5, 10 for Recall@10, MRR@10 and nDCG@10.
    pub cutoff: u32,
    /// Why no expected section ranks within it.
    #[serde(deserialize_with = "shape::name")]
    pub class: FailureClass,
    /// The routes that ran and whose trace found no expected section, in
    /// name order; the reranker finds nothing, so it is never one.
    pub missed_by: Vec<String>,
    /// The routes, the reranker among them, that could not run, each with
    /// its reason.
    #[serde(deserialize_with = "distinct_texts")]
    pub unavailable: BTreeMap<String, String>,
}

/// Why no expected section ranks within a cut-off. A wrong answer is the
/// class of `ask` (T035), not of retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FailureClass {
    /// No passage of the bundle holds an expected section: fix extraction,
    /// chunking or retrieval.
    NotRetrieved,
    /// A passage holds one, ranked below the cut-off: fix fusion, reranking
    /// or the selection of context.
    Misranked,
}

/// The metrics of a run, or the differences of two: each an estimate with
/// its interval, absent when it covers no question.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Metrics {
    /// The share of answerable questions with an expected section in the top
    /// 5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub recall_at_5: Option<Estimate>,
    /// The share of answerable questions with an expected section in the top
    /// 10.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub recall_at_10: Option<Estimate>,
    /// The mean over answerable questions of the reciprocal rank of the
    /// first expected section, 0 below the top 10.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub mrr_at_10: Option<Estimate>,
    /// The mean over answerable questions of the normalized discounted
    /// cumulative gain in the top 10, each expected section gaining 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub ndcg_at_10: Option<Estimate>,
    /// The share of unanswerable questions whose bundle holds no passage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub no_answer_accuracy: Option<Estimate>,
    /// The share of answerable questions whose bundle holds no passage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub false_abstentions: Option<Estimate>,
    /// The median latency of a retrieval, in microseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub latency_p50_us: Option<Estimate>,
    /// The 95th percentile of a retrieval's latency, in microseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "estimate")]
    pub latency_p95_us: Option<Estimate>,
}

/// A metric's value over every question, and its 95 % interval.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Estimate {
    /// The value over every question.
    pub value: f64,
    /// The interval's low end: the 2.5th percentile of the resamples.
    pub low: f64,
    /// The interval's high end: the 97.5th percentile of the resamples.
    pub high: f64,
}

/// An estimate, from a JSON object only: a metric that covers no question is
/// left out, never written `null`.
fn estimate<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Estimate>, D::Error> {
    shape::object(deserializer).map(Some)
}

/// Texts by name, from a JSON object that names each once: a map alone would
/// keep the last text of a name given twice.
fn distinct_texts<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    deserializer.deserialize_map(TextsVisitor)
}

/// Reads texts by name from a JSON object, refusing a name given twice.
struct TextsVisitor;

impl<'de> Visitor<'de> for TextsVisitor {
    type Value = BTreeMap<String, String>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an object of names and texts, each name once")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
        let mut texts = BTreeMap::new();
        while let Some((name, text)) = access.next_entry::<String, String>()? {
            if texts.contains_key(&name) {
                return Err(de::Error::custom(format_args!(
                    "the name {name} is given twice"
                )));
            }
            texts.insert(name, text);
        }
        Ok(texts)
    }
}
