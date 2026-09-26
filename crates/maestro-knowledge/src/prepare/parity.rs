//! The native profile's parity fixtures, `native-parity.json`: complete
//! inputs, each with the ordered IDs the native counter gives it. Only the
//! native counter records them, maestro-canonicalization's `NativeTokenizer`
//! under the contract ID the file's `profile` names. A test fails when that
//! ID is not the committed native profile's, and `just native` when the live
//! native counter disagrees with any of them.
//!
//! A fixture is a JSON object: its `name`; its `input`, the concatenation of
//! its parts; its `ids`, the concatenation of its runs; and `canary`, true
//! for the few a tokenizer's `verify` tokenizes again. A part is a string and
//! a run an ID, or either one repeated:
//!
//! ```json
//! {"name": "plain_499", "input": [{"repeat": "a ", "times": 497}],
//!  "ids": [0, {"repeat": 10, "times": 497}, 2]}
//! ```

use serde::Deserialize;
use std::iter;

/// The fixtures built into this crate.
const FIXTURES: &str = include_str!("native-parity.json");

/// One parity fixture: a complete input and the native counter's ordered IDs
/// for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Fixture {
    /// The name a refusal gives.
    pub(super) name: String,
    /// The complete input.
    pub(super) input: String,
    /// The native counter's IDs for it, in order.
    pub(super) ids: Vec<u32>,
    /// Whether a tokenizer's `verify` tokenizes it again.
    pub(super) canary: bool,
}

/// The fixtures built into this crate, in their order.
///
/// # Errors
///
/// When they are not valid JSON of their shape.
pub(super) fn fixtures() -> Result<Vec<Fixture>, serde_json::Error> {
    parse(FIXTURES)
}

/// The fixtures `text` holds, in their order.
pub(super) fn parse(text: &str) -> Result<Vec<Fixture>, serde_json::Error> {
    let file: ParityJson = serde_json::from_str(text)?;
    Ok(file
        .fixtures
        .into_iter()
        .map(|fixture| Fixture {
            name: fixture.name,
            input: expand(fixture.input).collect(),
            ids: expand(fixture.ids).collect(),
            canary: fixture.canary,
        })
        .collect())
}

/// Each run's item, as many times as the run repeats it.
fn expand<T: Clone>(runs: Vec<Run<T>>) -> impl Iterator<Item = T> {
    runs.into_iter().flat_map(|run| match run {
        Run::Once(item) => iter::repeat_n(item, 1),
        Run::Repeated { repeat, times } => iter::repeat_n(repeat, times),
    })
}

/// The file as it is written; its `profile` is the tests' to check.
#[derive(Deserialize)]
struct ParityJson {
    /// The fixtures, in their order.
    fixtures: Vec<FixtureJson>,
}

/// A fixture as the file writes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureJson {
    /// See [`Fixture::name`].
    name: String,
    /// See [`Fixture::canary`]; false when absent.
    #[serde(default)]
    canary: bool,
    /// The parts of [`Fixture::input`].
    input: Vec<Run<String>>,
    /// The runs of [`Fixture::ids`].
    ids: Vec<Run<u32>>,
}

/// A part of an input or a run of IDs: one item, or one item repeated.
#[derive(Deserialize)]
#[serde(untagged)]
enum Run<T> {
    /// The item once.
    Once(T),
    /// `repeat`, `times` times.
    Repeated {
        /// The item.
        repeat: T,
        /// How many times.
        times: usize,
    },
}
