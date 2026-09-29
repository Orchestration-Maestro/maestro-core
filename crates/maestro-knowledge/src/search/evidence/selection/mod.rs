//! Budgeted MMR selection over validated source windows.

mod algorithm;
mod relevant;
mod render;
mod types;

pub(crate) use algorithm::select;
pub(crate) use types::{SelectionBudget, SelectionCandidate, SelectionResult};
