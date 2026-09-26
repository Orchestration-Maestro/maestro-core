//! The knowledge pipeline of Maestro (docs/architecture/01): collections, their
//! sources and the corpora they import, composed over the kernel.
//!
//! It starts with the two public contracts S1 imports a corpus through, each
//! parsed strictly into typed values (ADR-0014): a collection's declaration,
//! `maestro-collection/1` ([`collection`]), and a corpus manifest,
//! `maestro-corpus/1`, one JSON line per document ([`corpus`]). Neither holds a
//! machine path: a declaration names its corpus by a binding the kernel
//! resolves ([`maestro_kernel::binding`]), and every path either gives is a
//! [`RelativePath`], which stays inside its directory.
//!
//! Preparing the imported revisions for search starts with counting tokens as
//! the selected embedder counts them, through the model router ([`prepare`]).

pub mod collection;
pub mod corpus;
pub mod prepare;
mod relative_path;
mod shape;
pub mod suite;

pub use relative_path::RelativePath;
