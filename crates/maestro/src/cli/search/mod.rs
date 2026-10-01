//! CLI presentation for scoped search bundles.

mod execution;
mod presentation;
#[cfg(test)]
mod tests;

pub(super) use execution::{invalid_request, ports, run};
#[cfg(test)]
use presentation::search_document;
