//! Digest-bound delivery graphs and source-scoped persistence.

mod coverage;
mod error;
mod group_parts;
mod hierarchy;
mod mapping;
mod membership;
mod read;
mod record;
mod serialization;
mod types;
mod validation;

pub use error::Error;
pub use mapping::{MappingEntry, MappingLedger};
pub use types::{
    ContextKind, ContextRelation, DeliveryGraph, DeliveryUnit, Exclusion, FamilyKey,
    GraphDescriptor, Group, GroupKind, MappingContribution, Part, RankPolicy, RetrievalMembership,
    RetrievalView, SourceRange, SplitMarker, UnitKind,
};

#[cfg(test)]
mod tests;

pub use read::GraphKey;
