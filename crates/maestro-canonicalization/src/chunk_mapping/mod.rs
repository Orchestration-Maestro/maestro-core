//! Phase B-only mappings; original source offsets are never rendered offsets.
pub(crate) use document::map_document;
pub(crate) use slice::{map_accounting, mapped_slice};

mod document;
mod refusal;
mod slice;
#[cfg(test)]
mod tests;
