//! Authoritative section reads and bounded evidence assembly.

/// Runs authoritative assembly under the inherited deadline.
mod assemble;
/// Counts compact serialized passages with the selected counter.
mod budget;
/// Finds the documents of a chunk set by `source_ref`.
mod chunk_set_documents;
/// Detects explicit structured disagreements in candidate tables.
pub(super) mod conflicts;
/// Shares manifest-authorized identity and section occurrence across helpers.
mod families;
/// Computes case-sensitive shingles and material-difference signatures.
mod features;
/// Reads one authorized canonical section from a completed chunk set.
mod section_reader;
/// Derives canonical section extents and sibling-safe evidence windows.
mod sections;
/// Applies atomic MMR and source-window selection under serialized budgets.
mod selection;
/// Orders passages and derives trace metadata and known gaps.
mod signals;
/// Loads and caches authorized canonical source records.
mod source;
/// Unions candidate source spans without crossing revisions.
mod spans;
/// Exercises the helper contracts with canonical source fixtures.
#[cfg(test)]
mod tests;
/// Shared error types for authoritative evidence assembly.
mod types;
pub(crate) use sections::SectionIndex;
/// Compares numeric version components without integer conversion.
mod versions;

pub use assemble::deadline::assemble_evidence;
pub use chunk_set_documents::ChunkSetDocuments;
pub use section_reader::{SectionExcerpt, SectionReadError, read_section};
pub use types::{EvidenceCounter, EvidenceError};
