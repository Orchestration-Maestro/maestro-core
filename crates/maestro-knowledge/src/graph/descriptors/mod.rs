//! Deterministic, disposable source descriptors beside passage projections.

mod build;
mod embedding;
mod port;
mod qdrant;
mod source;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_backend;
#[cfg(test)]
mod tests_embedding;
#[cfg(test)]
mod tests_namespace;
#[cfg(test)]
mod tests_projection;
#[cfg(test)]
mod tests_source;
mod types;

pub use build::{DescriptorInput, build};
pub use port::{DescriptorEmbedder, DescriptorProjection, DescriptorQuery, EmbeddedDescriptors};
pub use qdrant::DescriptorQdrant;
pub use types::{
    BUILDER_VERSION, Descriptor, DescriptorError, DescriptorPin, DescriptorProfile,
    DescriptorReceipt, SourcePointer,
};
