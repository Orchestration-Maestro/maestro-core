//! Budgeted MMR selection over validated source windows.

mod algorithm;
mod relevant;
mod render;
mod types;
mod units;

pub(crate) use algorithm::select;
pub(crate) use types::{SelectionBudget, SelectionCandidate, SelectionResult};
