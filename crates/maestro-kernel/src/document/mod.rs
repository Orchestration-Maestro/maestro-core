//! The pipeline's document records (building block B5; docs/architecture/01
//! §11, plan D6): the collections and the sources they declare, the documents
//! each source holds, and the revisions of each document.
//!
//! A collection and a source follow their declaration: recording one again
//! takes its new values. A document keeps the collection, source and source
//! reference it was first recorded with. Its id, the namespaced hash of its
//! source reference (01 §2.1), comes from the import, as a revision's id,
//! canonicalization's recipe over its bytes and metadata, does.
//!
//! A revision is one exact version of a document's bytes and metadata,
//! recorded once, in the transaction that pins its original and canonical
//! artifacts. Triggers refuse any change to its content afterwards: only its
//! status, canonicalization's verdict, moves, and only to `failed`, which it
//! never leaves. A failed revision stays readable and is never eligible.
//!
//! The migration `0004_documents` creates every table of the pipeline's
//! records, those the quality gate and the preparation write included, so no
//! later task of S1 needs a migration of its own.

mod collection;
mod error;
mod revision;
#[cfg(test)]
mod tests;

pub use collection::{Collection, Document, Source};
pub use error::Error;
pub use revision::{Recorded, Revision, RevisionStatus};
