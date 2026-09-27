//! Canonical section indexing and sibling-safe evidence windows.

mod index;
mod validation;

#[cfg(test)]
pub(crate) use index::SectionIndex;
pub(super) use validation::{block_span, contains, valid_span};
