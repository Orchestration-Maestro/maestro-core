//! Phase B chunk batches: assembly, prepared-input identities and replay validation; none of
//! these records grant access.
mod batch;
mod build;
mod chrome;
mod identity;
#[cfg(test)]
mod tests;
mod validation;

pub use batch::{ChunkBatch, ChunkDocument, PreparedInputGroup, RetrievalChunk, UnitCoverage};
pub use build::chunk_documents;
pub use chrome::{ChromeCount, chrome_spans, indexed_title, left_out_chrome};
