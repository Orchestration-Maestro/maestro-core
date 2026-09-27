//! The kernel of Maestro: the single authoritative store every later
//! capability composes (docs/architecture/04 §3, building blocks B1–B11).
//!
//! It provides the content-addressed artifact store, database, journal, scopes,
//! jobs, document and generation records, evidence, retrieval, model gateway,
//! model-card/evaluation/selection registry, telemetry and capability registry
//! for higher-level workflows.

pub mod artifact;
pub mod binding;
pub mod capability;
pub mod chunk_set;
pub mod document;
pub mod eval;
pub mod evidence;
mod filesystem;
pub mod gateway;
pub mod generation;
pub mod job;
pub mod journal;
/// Scoped model identities, immutable evaluations, and explicit selections.
///
/// An evaluation with no generation is valid for failed preflight. Its report
/// and manifest are opaque, digest-verified artifacts. `eligible` means every
/// hard constraint was measured and passed; `ineligible` means a measured
/// constraint failed; `blocked` means required proof was missing; `failed`
/// means the run or report failed; `interrupted` means the run did not finish.
/// Only an eligible `real` evaluation can be selected. Synthetic CI runs,
/// blocked or failed runs, and v1 cards are never winners. T030b must assign
/// these dispositions only after applying those semantics.
pub mod model;
pub mod paths;
pub mod retrieval;
pub mod scope;
pub mod store;
pub mod telemetry;
