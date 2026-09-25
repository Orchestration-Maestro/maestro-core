//! Derived Phase B coordinates and evidence; none of these records grant access.
mod batch;
mod build;
mod identity;
mod mapping;
mod prepared;
#[cfg(test)]
mod tests;
mod validation;

pub use batch::{ChunkBatch, ChunkDocument, PreparedInputGroup, RetrievalChunk, UnitCoverage};
pub use build::chunk_documents;
pub use mapping::{
    AccountingDisposition, CHUNKER_VERSION, InlineEnvelope, MappedDocument, MappingRun, OriginMode,
    SourceDisposition, SourceOrigin, SourceUnit, TextRange, UnitField,
};
pub use prepared::{
    ChunkContent, Contribution, Fragment, InputPart, InputRole, PREPARATION_PROFILE, SplitKind,
    TableWindow,
};
