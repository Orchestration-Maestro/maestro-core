//! Canonical section indexing and sibling-safe evidence windows.

mod index;
#[cfg(test)]
mod tests;
mod validation;

pub(super) use index::{Expansion, SectionIndex};
pub(super) use validation::{block_span, contains, valid_span};
