//! Bundles: `maestro-evidence/1`, the search response contract, checked whole
//! when read.

use super::passage::Passage;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// What a search returns, `maestro-evidence/1` (docs/architecture/02 §6):
/// the passages it cites, what qualifies and bounds them, and, apart from
/// them, the trace of how each was found and ranked. Read from JSON, it is
/// checked whole, as the module says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Unchecked")]
pub struct Bundle {
    /// The contract it follows.
    pub schema: Schema,
    /// The collection it searched.
    pub collection: String,
    /// The generation of that collection the search was pinned to.
    pub generation: i64,
    /// The question, as the caller asked it.
    pub query: String,
    /// The question's language, such as `fr`.
    pub lang: String,
    /// Whether each route, and the reranker as `rerank`, ran, by name.
    pub routes: BTreeMap<String, RouteStatus>,
    /// The passages it cites, in reading order.
    pub passages: Vec<Passage>,
    /// The passages that state different values of one attribute.
    pub conflicts: Vec<Conflict>,
    /// The evidence required but not found or not accessible, each in words.
    pub known_gaps: Vec<String>,
    /// The tokens its passages take, and the most they could.
    pub budget: Budget,
    /// How each passage was found and ranked, apart from the evidence.
    pub trace: Vec<Trace>,
}

/// The contract a bundle follows; this version writes and reads the first
/// only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Schema {
    /// `maestro-evidence/1`.
    #[serde(rename = "maestro-evidence/1")]
    V1,
}

/// Whether a route or the reranker ran for a search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteStatus {
    /// It ran, written `"ok"`.
    Ok,
    /// It could not run, for the reason given, written
    /// `{"unavailable": "<reason>"}`.
    Unavailable(String),
}

/// Passages that state different values of one attribute of one entity,
/// such as a default port that changed between versions: each is kept, and
/// flagged here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conflict {
    /// What the values are of, such as `agent`.
    pub entity: String,
    /// Which of its attributes, such as `default port`.
    pub attribute: String,
    /// The numbers of the passages that state them.
    pub passages: Vec<u32>,
}

/// A search's evidence budget, in tokens of the answerer's tokenizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    /// The tokens its passages take.
    pub evidence_tokens: u32,
    /// The most they could take.
    pub limit: u32,
}

/// How one passage was found and ranked: signals about the evidence, kept
/// apart from it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trace {
    /// The number of the passage it traces.
    pub n: u32,
    /// The reranker's score, absent when the reranker could not run: it
    /// orders passages, and is neither a probability nor a confidence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// The routes that found the passage.
    pub routes: Vec<String>,
    /// Whether the passage is a procedure.
    pub procedural: bool,
}

/// A bundle as its JSON holds it, before the checks across its parts; each
/// field is the [`Bundle`] field of its name.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Unchecked {
    /// [`Bundle::schema`].
    schema: Schema,
    /// [`Bundle::collection`].
    collection: String,
    /// [`Bundle::generation`].
    generation: i64,
    /// [`Bundle::query`].
    query: String,
    /// [`Bundle::lang`].
    lang: String,
    /// [`Bundle::routes`].
    routes: BTreeMap<String, RouteStatus>,
    /// [`Bundle::passages`].
    passages: Vec<Passage>,
    /// [`Bundle::conflicts`].
    conflicts: Vec<Conflict>,
    /// [`Bundle::known_gaps`].
    known_gaps: Vec<String>,
    /// [`Bundle::budget`].
    budget: Budget,
    /// [`Bundle::trace`].
    trace: Vec<Trace>,
}

impl TryFrom<Unchecked> for Bundle {
    type Error = String;

    /// The bundle `unchecked` holds, once its passages are numbered from 1,
    /// each number given once, and its conflicts and trace name only its
    /// passages.
    fn try_from(unchecked: Unchecked) -> Result<Self, String> {
        let numbers = numbers(&unchecked.passages)?;
        for conflict in &unchecked.conflicts {
            if let Some(n) = conflict.passages.iter().find(|n| !numbers.contains(n)) {
                return Err(format!(
                    "the conflict on {}'s {} names passage {n}, which the bundle does not hold",
                    conflict.entity, conflict.attribute
                ));
            }
        }
        if let Some(trace) = unchecked
            .trace
            .iter()
            .find(|trace| !numbers.contains(&trace.n))
        {
            return Err(format!(
                "the trace names passage {}, which the bundle does not hold",
                trace.n
            ));
        }
        Ok(Self {
            schema: unchecked.schema,
            collection: unchecked.collection,
            generation: unchecked.generation,
            query: unchecked.query,
            lang: unchecked.lang,
            routes: unchecked.routes,
            passages: unchecked.passages,
            conflicts: unchecked.conflicts,
            known_gaps: unchecked.known_gaps,
            budget: unchecked.budget,
            trace: unchecked.trace,
        })
    }
}

/// The numbers of `passages`, each from 1 and given once.
fn numbers(passages: &[Passage]) -> Result<BTreeSet<u32>, String> {
    let mut numbers = BTreeSet::new();
    for passage in passages {
        if passage.n == 0 {
            return Err("passage number 0: passages are numbered from 1".to_owned());
        }
        if !numbers.insert(passage.n) {
            return Err(format!("passage number {} is given twice", passage.n));
        }
    }
    Ok(numbers)
}
