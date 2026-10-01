//! Budgeted MMR selection over validated source windows.

mod algorithm;
mod mmr;
mod relevant;
mod render;
mod render_cluster;
mod trial;
mod types;

pub(crate) use super::selection_candidate::SelectionCandidate;
pub(crate) use algorithm::select;
pub(crate) use types::{SelectionBudget, SelectionResult};
