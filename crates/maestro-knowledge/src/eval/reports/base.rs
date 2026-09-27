//! Reports: `maestro-eval-report/1`, what a run measured, question by
//! question and metric by metric, read back as strictly as it is written.

use super::{
    metrics::Metrics,
    v2::{
        CohortStatistics, ItemStatus, MeasurementCohort, Subgroup, SubgroupStatistics, TrialMode,
    },
};
use crate::shape;
use maestro_kernel::artifact::Digest;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, MapAccess, Visitor},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    marker::PhantomData,
    num::NonZeroU32,
    str::FromStr,
};

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

/// Frozen inputs for one v2 route attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderV2 {
    /// The suite, collection, generation, profiles and bootstrap seed.
    pub header: Header,
    /// Stable run ID shared by all candidate/route attempts.
    pub run_id: String,
    /// Stable candidate label in the frozen manifest.
    pub candidate_id: String,
    /// Exact T030a card registration ULID.
    pub card_id: String,
    /// Exact v2 card artifact digest.
    pub card_digest: Digest,
    /// Exact frozen bake-off manifest digest.
    pub manifest_digest: Digest,
    /// Planned scored repetitions, 1 through this count.
    pub planned_repetitions: u32,
    /// Planned warm-up attempts before scored work.
    pub planned_warm_ups: u32,
    /// Digest of the frozen collection revisions and quality decisions.
    pub corpus_digest: Digest,
    /// Digest of the frozen canonical and original inputs.
    pub input_digest: Digest,
    /// One-based durable attempt number.
    pub attempt: u32,
    /// Scored repetition number, zero for warm-ups.
    pub repetition: u32,
    /// Original attempt if this is an explicitly authorized retry.
    pub retry_of: Option<u32>,
    /// Seed used for this route attempt.
    pub attempt_seed: u64,
    /// Independent route measured by this attempt.
    pub route: String,
    /// Warm-up work excluded from candidate ranking.
    pub warm_up: bool,
    /// Explicitly labelled cross-lingual question IDs from the frozen manifest.
    pub cross_lingual_questions: BTreeSet<String>,
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
    /// The digest of the text the suite was read from, its file's
    /// ([`Suite::digest`](crate::suite::Suite::digest)).
    #[serde(serialize_with = "hexadecimal", deserialize_with = "shape::digest")]
    pub suite_digest: Digest,
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
    /// Stable run ID shared by all candidate/route attempts in v2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// Stable candidate label in the frozen manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_id: Option<String>,
    /// Exact T030a card registration ULID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_id: Option<String>,
    /// Exact v2 card artifact digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_digest: Option<Digest>,
    /// Exact frozen bake-off manifest digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_digest: Option<Digest>,
    /// Planned scored repetitions, 1 through this count in v2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_repetitions: Option<u32>,
    /// Planned warm-up attempts before scored work in v2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_warm_ups: Option<u32>,
    /// Frozen corpus identity for v2; absent in legacy v1 reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corpus_digest: Option<Digest>,
    /// Frozen canonical/original input identity for v2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_digest: Option<Digest>,
    /// Synthetic or real provenance for v2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<TrialMode>,
    /// Whether any item violated collection/generation integrity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity_violation: Option<bool>,
    /// Observed latency/cost summaries by measured cohort.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "distinct_cohorts"
    )]
    pub cohorts: Option<BTreeMap<MeasurementCohort, CohortStatistics>>,
    /// Frozen FR, EN and explicitly labelled cross-lingual summaries.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "distinct_subgroups"
    )]
    pub subgroups: Option<BTreeMap<Subgroup, SubgroupStatistics>>,
}

impl FromStr for Report {
    type Err = serde_json::Error;

    /// The report `text` holds: one JSON object and nothing after it.
    fn from_str(text: &str) -> Result<Self, serde_json::Error> {
        let report: Self = shape::parse(text)?;
        report.validate_version()?;
        Ok(report)
    }
}

/// The contract a report follows; this version writes and reads the first
/// only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Schema {
    /// `maestro-eval-report/1`.
    #[serde(rename = "maestro-eval-report/1")]
    V1,
    /// `maestro-eval-report/2`.
    #[serde(rename = "maestro-eval-report/2")]
    V2,
}

/// What one question got: the rank of each section and document it expects,
/// the size of its bundle, how long its retrieval took, and its failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct QuestionResult {
    /// The question's id in its suite.
    pub id: String,
    /// Whether any section answers it.
    pub answerable: bool,
    /// The sections and documents it expects, in the suite's order, each
    /// with its rank.
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
    /// One-based attempt number, absent in v1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
    /// Scored repetition number, zero for warm-ups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repetition: Option<u32>,
    /// Original attempt if this is an explicitly authorized retry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_of: Option<u32>,
    /// Seed used for this attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Independent route measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    /// Typed item success or failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ItemStatus>,
    /// Full elapsed microseconds, not saturated to the legacy u32 field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_us: Option<u64>,
    /// Observed warm/cold/unknown/unavailable timing cohort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cohort: Option<MeasurementCohort>,
    /// Whether this was a declared warm-up, excluded from ranked samples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warm_up: Option<bool>,
    /// Question language (`fr` or `en`) in v2; absent in v1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Frozen-manifest cross-lingual subset membership.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cross_lingual: Option<bool>,
}

impl QuestionResult {
    /// The item's observed cohort, treating legacy v1 timing as unknown.
    #[must_use]
    pub fn measurement_cohort(&self) -> MeasurementCohort {
        self.cohort.unwrap_or(MeasurementCohort::Unknown)
    }

    /// The best rank of an expected section, if the bundle holds any.
    #[must_use]
    pub fn best_rank(&self) -> Option<u32> {
        self.expected
            .iter()
            .filter_map(|expected| expected.rank)
            .min()
            .map(NonZeroU32::get)
    }
}

/// A section a question expects, or a document without sections it expects
/// whole, and where its bundle ranked it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Expected {
    /// The ID of the section's document, or of the document expected whole,
    /// in the evaluated generation.
    pub document_id: String,
    /// The section's ID in the evaluated generation, absent when the
    /// document has no section and is expected whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_id: Option<String>,
    /// The other expected sections that are copies of this answer, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// The rank of its best passage, from 1, absent when no passage holds
    /// it: a report that gives 0 is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<NonZeroU32>,
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

/// `digest` as its 64 hexadecimal characters.
fn hexadecimal<S: Serializer>(digest: &Digest, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(digest.as_str())
}

/// Texts by name, from a JSON object that names each once: a map alone would
/// keep the last text of a name given twice.
fn distinct_texts<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    deserializer.deserialize_map(TextsVisitor)
}

/// Cohort statistics, from a JSON object that names each cohort once.
fn distinct_cohorts<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<BTreeMap<MeasurementCohort, CohortStatistics>>, D::Error> {
    distinct_map(deserializer).map(Some)
}

/// Subgroup statistics, from a JSON object that names each subgroup once.
fn distinct_subgroups<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<BTreeMap<Subgroup, SubgroupStatistics>>, D::Error> {
    distinct_map(deserializer).map(Some)
}

/// Reads a map without allowing duplicate keys to overwrite earlier evidence.
fn distinct_map<'de, D, K, V>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Ord + fmt::Debug,
    V: Deserialize<'de>,
{
    deserializer.deserialize_map(DistinctMap::<K, V>(PhantomData))
}

/// A visitor for a map whose keys may not repeat.
struct DistinctMap<K, V>(PhantomData<(K, V)>);

impl<'de, K, V> Visitor<'de> for DistinctMap<K, V>
where
    K: Deserialize<'de> + Ord + fmt::Debug,
    V: Deserialize<'de>,
{
    type Value = BTreeMap<K, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an object of unique keys and values")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
        let mut values = BTreeMap::new();
        while let Some((key, value)) = access.next_entry::<K, V>()? {
            if values.insert(key, value).is_some() {
                return Err(de::Error::custom("a map key is given twice"));
            }
        }
        Ok(values)
    }
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
