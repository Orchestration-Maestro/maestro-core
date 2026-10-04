//! `knowledge graph`: the commands over a collection's knowledge graph.

pub(super) mod attach;
pub(super) mod build;
pub(super) mod cleanup;
mod extract_output;
mod extractor;
mod failure;
mod job;
pub(super) mod rebuild;
#[cfg(feature = "engine")]
mod rebuild_descriptors;
#[cfg(feature = "engine")]
mod rebuild_work;
#[cfg(test)]
mod tests;
