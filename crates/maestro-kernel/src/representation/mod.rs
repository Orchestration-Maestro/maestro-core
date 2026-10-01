//! Immutable representation manifests and one-time generation binding.
mod error;
mod read;
mod record;
mod types;
mod validation;

pub use error::Error;
pub use types::{
    RepresentationKey, RepresentationLayout, RepresentationMember, RepresentationSet,
    RepresentationSetSpec, RepresentationShard, RepresentationState,
};
