//! The pipeline's document records (building block B5; docs/architecture/01
//! §11, plan D6): the collections and the sources they declare, the documents
//! each source holds, the revisions of each document, and the quality
//! disposition of each revision.
//!
//! A collection and a source follow their declaration: recording one again
//! takes its new values. Their ids are scope names
//! ([`check_name`](crate::scope::check_name)), and the kernel refuses any
//! other before it writes, so each id forms one segment of its scope's path
//! and no record reaches into another's scope.
//!
//! A document keeps the collection, source and source reference it was first
//! recorded with, and a source reference names one document in each
//! collection. Its id, the namespaced hash of its collection and its source
//! reference (01 §2.1), comes from the import, as a revision's id,
//! canonicalization's recipe over its bytes and metadata, does.
//!
//! A revision is one exact version of a document's bytes and metadata,
//! recorded once, in the transaction that pins its original and canonical
//! artifacts. Triggers refuse any change to its content afterwards, and its
//! replacement or deletion, whoever writes: only its status,
//! canonicalization's verdict, moves, and only to `failed`, which it never
//! leaves. A failed revision stays readable and is never eligible.
//!
//! A revision's quality disposition (01 §4) is decided once: recording
//! another keeps the first. One that holds the revision back,
//! `needs_reextraction`, `quarantined` or `excluded`, is journaled in the
//! same write as `maestro.knowledge.revision.held.v1`, on the stream of the
//! revision's collection ([`stream`](crate::journal::stream)). A revision
//! that must never be read undecided, as the import's holds, is recorded
//! with its disposition in one write.
//!
//! Every reader takes the caller's [`ScopeSet`](crate::scope::ScopeSet) and
//! reads only what it covers: a collection has the scope
//! `workspace/default/collection/<id>`, a source `…/source/<id>` below it,
//! and a document and its revisions have their source's.
//!
//! The migration `0004_documents` creates every table of the pipeline's
//! records, those the quality gate and the preparation write included, so no
//! later task of S1 needs a migration of its own.

mod collection;
mod disposition;
mod error;
mod revision;
#[cfg(test)]
mod tests;

pub use collection::{Collection, Document, Source};
pub use disposition::{Disposition, Outcome};
pub use error::Error;
pub use revision::{Recorded, Revision, RevisionStatus};
