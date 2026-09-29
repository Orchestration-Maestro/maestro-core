//! Opt-in /4 mapped delivery graph and digest-pinned ranking view.
mod build;
mod build_units;
mod group_helpers;
mod group_links;
mod group_validation;
mod groups;
mod ledger;
mod prepared;
mod profile;
mod retrieval_validation;
mod serialization;
mod table_packing;
mod types;
mod validation;

#[cfg(test)]
mod tests;

pub use build::unit_documents;
pub use profile::{RankedUnit, UnitProfile, UnitSizeLimits};
pub use serialization::{serialize_graph, serialize_mapping};
pub use types::{
    ContextRelation, CoverageEntry, DeliveryGraph, DeliveryUnit, Exclusion, FamilyKey,
    GraphDescriptor, Group, GroupKind, MappingArtifact, MappingContribution, MappingEntry,
    PartRole, RetrievalMembership, RetrievalView, SourcePart, SourceRange, SplitMarker, UnitBatch,
    UnitGraphInput, UnitKind,
};
