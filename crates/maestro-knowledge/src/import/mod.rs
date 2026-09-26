//! Importing a collection's corpus through its `maestro-corpus/1`
//! manifests, instead of acquiring it again (docs/architecture/01 §2.1, plan
//! D6, FR-S1-002).
//!
//! [`import`] records the collection and its sources, whose scopes are
//! `workspace/default/collection/<id>/source/<id>`, before any revision.
//! Then it streams each source's manifest, one line and one document at a
//! time: each line is parsed strictly, its document read relative to the
//! manifest's own directory and checked against the line's digest and
//! size, then canonicalized; its original and its canonical document are
//! stored as artifacts, and its revision recorded. The report counts every
//! line once, as imported, unchanged, held or refused, and the import
//! journals the same counts as `maestro.knowledge.import.completed.v1`.
//!
//! A document's ID is `doc-` followed by the SHA-256 of `collection`, its
//! collection's ID and its `source_ref`, separated by NUL bytes: one URL in
//! two collections is two documents, and a document keeps its ID from one
//! release to the next. A revision's ID is canonicalization's recipe over
//! the document's ID, its bytes and every field of its line that describes
//! it, so a change of metadata alone gives a new revision.
//!
//! Lines of one manifest that give a `source_ref` different digests are all
//! recorded, each a revision of the one document, and each is held back:
//! quarantined, decided by `import`, with its reason, and journaled as
//! `maestro.knowledge.revision.held.v1`, so neither silently replaces the
//! other. The quality gate leaves a revision that has a disposition alone.
//!
//! A line that is no entry, a document that cannot be read or is not what
//! its line declares, and bytes or a document the kernel holds otherwise
//! are refused, each with its line and a typed [`Reason`]; the rest of the
//! manifest is imported all the same. A second import finds every revision
//! unchanged and writes nothing but its completion, and a rerun after a
//! failure continues where the first stopped.

mod collection;
mod corpus;
mod entry;
mod error;
mod report;
mod source;
#[cfg(test)]
mod tests;

pub use collection::import;
pub use error::Error;
pub use report::{Reason, Refusal, Report};
