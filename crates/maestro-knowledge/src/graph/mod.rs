//! The knowledge graph's construction (specs/002-knowledge-graph): claims
//! extracted from canonical blocks by strict, data-only rules, each quote
//! located in the revision's original bytes before the kernel admits it.

pub mod build;
pub mod rules;
mod structure;
#[cfg(test)]
mod tests;
pub mod verify;
