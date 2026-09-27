//! Controlled, scope-bound retrieval and its shared literal search rules.

mod identifiers;
#[cfg(test)]
mod tests;

pub use identifiers::{contains_identifier, normalize_whitespace};
