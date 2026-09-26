//! Bundles: `maestro-evidence/1`, the search response contract, checked whole
//! when written and when read.

use super::passage::Passage;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, MapAccess, Visitor},
    ser,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// What a search returns, `maestro-evidence/1` (docs/architecture/02 §6):
/// the passages it cites, what qualifies and bounds them, and, apart from
/// them, the trace of how each was found and ranked. It is checked whole when
/// written and when read, as the module says.
#[derive(Debug, Clone, PartialEq, Deserialize)]
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
    /// It could not run, for the reason given, which a bundle never leaves
    /// blank, written `{"unavailable": "<reason>"}`.
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
    /// The numbers of the passages that state them: at least two.
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
    /// The reranker's score, a finite number, absent when the reranker could
    /// not run: it orders passages, and is neither a probability nor a
    /// confidence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// The routes that found the passage.
    pub routes: Vec<String>,
    /// Whether the passage is a procedure.
    pub procedural: bool,
}

impl Serialize for Bundle {
    /// Writes the bundle once its checks pass: one that would not read back
    /// is refused before anything is written.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        check(self).map_err(ser::Error::custom)?;
        let Self {
            schema,
            collection,
            generation,
            query,
            lang,
            routes,
            passages,
            conflicts,
            known_gaps,
            budget,
            trace,
        } = self;
        Written {
            schema,
            collection,
            generation,
            query,
            lang,
            routes,
            passages,
            conflicts,
            known_gaps,
            budget,
            trace,
        }
        .serialize(serializer)
    }
}

/// A bundle as its JSON writes it, borrowed once its checks passed; each
/// field is the [`Bundle`] field of its name.
#[derive(Serialize)]
struct Written<'bundle> {
    /// [`Bundle::schema`].
    schema: &'bundle Schema,
    /// [`Bundle::collection`].
    collection: &'bundle str,
    /// [`Bundle::generation`].
    generation: &'bundle i64,
    /// [`Bundle::query`].
    query: &'bundle str,
    /// [`Bundle::lang`].
    lang: &'bundle str,
    /// [`Bundle::routes`].
    routes: &'bundle BTreeMap<String, RouteStatus>,
    /// [`Bundle::passages`].
    passages: &'bundle [Passage],
    /// [`Bundle::conflicts`].
    conflicts: &'bundle [Conflict],
    /// [`Bundle::known_gaps`].
    known_gaps: &'bundle [String],
    /// [`Bundle::budget`].
    budget: &'bundle Budget,
    /// [`Bundle::trace`].
    trace: &'bundle [Trace],
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
    /// [`Bundle::routes`], each named once.
    #[serde(deserialize_with = "distinct_routes")]
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

    /// The bundle `unchecked` holds, once its checks pass.
    fn try_from(unchecked: Unchecked) -> Result<Self, String> {
        let bundle = Self {
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
        };
        check(&bundle)?;
        Ok(bundle)
    }
}

/// Why the parts of `bundle` disagree, if they do, the checks it passes when
/// written and when read: no span starts after it ends; its passages are
/// numbered from 1, each number given once; each route that could not run
/// says why; each conflict names at least two of its passages and no other;
/// and its trace names each of its passages at most once, and no other, with
/// a finite score if any.
fn check(bundle: &Bundle) -> Result<(), String> {
    for passage in &bundle.passages {
        passage.span.checked()?;
    }
    let numbers = numbers(&bundle.passages)?;
    for (name, status) in &bundle.routes {
        if matches!(status, RouteStatus::Unavailable(reason) if reason.trim().is_empty()) {
            return Err(format!(
                "the route {name} could not run and gives no reason"
            ));
        }
    }
    for conflict in &bundle.conflicts {
        check_conflict(conflict, &numbers)?;
    }
    check_trace(&bundle.trace, &numbers)
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

/// Why `conflict` is not one between passages of `numbers`, if it is not: it
/// names at least two of them, and no other.
fn check_conflict(conflict: &Conflict, numbers: &BTreeSet<u32>) -> Result<(), String> {
    let named: BTreeSet<u32> = conflict.passages.iter().copied().collect();
    if let Some(n) = named.difference(numbers).next() {
        return Err(format!(
            "the conflict on {}'s {} names passage {n}, which the bundle does not hold",
            conflict.entity, conflict.attribute
        ));
    }
    if named.len() < 2 {
        return Err(format!(
            "the conflict on {}'s {} names fewer than two passages",
            conflict.entity, conflict.attribute
        ));
    }
    Ok(())
}

/// Why `trace` does not trace passages of `numbers`, if it does not: each
/// entry names one of them, none twice, and scores it with a finite number if
/// it scores it at all.
fn check_trace(trace: &[Trace], numbers: &BTreeSet<u32>) -> Result<(), String> {
    let mut traced = BTreeSet::new();
    for entry in trace {
        if !numbers.contains(&entry.n) {
            return Err(format!(
                "the trace names passage {}, which the bundle does not hold",
                entry.n
            ));
        }
        if !traced.insert(entry.n) {
            return Err(format!("the trace names passage {} twice", entry.n));
        }
        if let Some(score) = entry.score.filter(|score| !score.is_finite()) {
            return Err(format!(
                "the trace scores passage {} {score}, which is not a finite number",
                entry.n
            ));
        }
    }
    Ok(())
}

/// The routes of a bundle, each named once, from a JSON object: a map alone
/// would keep the last status of a route named twice.
fn distinct_routes<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, RouteStatus>, D::Error> {
    deserializer.deserialize_map(RoutesVisitor)
}

/// Reads a bundle's routes from a JSON object, refusing a route named twice.
struct RoutesVisitor;

impl<'de> Visitor<'de> for RoutesVisitor {
    type Value = BTreeMap<String, RouteStatus>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an object of routes and their statuses")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
        let mut routes = BTreeMap::new();
        while let Some(name) = access.next_key::<String>()? {
            if routes.contains_key(&name) {
                return Err(de::Error::custom(format_args!(
                    "the route {name} is given twice"
                )));
            }
            let status = access.next_value()?;
            routes.insert(name, status);
        }
        Ok(routes)
    }
}
