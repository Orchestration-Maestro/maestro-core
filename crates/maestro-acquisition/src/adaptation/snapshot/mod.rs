//! Scoped snapshot envelope, reader and verified payloads.
mod payloads;
mod reader;
pub use reader::{ProcessingSnapshot, ResolvedSnapshot, SnapshotReader, SnapshotSchema};
