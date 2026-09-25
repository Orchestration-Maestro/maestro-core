//! Structural checks against the preserved bytes; no guessed repairs.
mod blocks;
mod report;
mod sections;
mod structure;
#[cfg(test)]
mod tests;

pub(crate) use structure::validate_structure;
