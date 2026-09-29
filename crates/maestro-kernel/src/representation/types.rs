//! Immutable revision shards and representation-set metadata.
use crate::artifact::Digest;
use serde::{Deserialize, Serialize};

/// Exact dense/sparse configuration represented by a set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepresentationLayout {
    /// Explicit layout version, interpreted by the publishing adapter.
    pub version: String,
    /// Dense embedding profile, matching the generation.
    pub embedding_profile: String,
    /// Sparse profile, matching the generation.
    pub sparse_profile: String,
}

/// One retrieval membership in a revision shard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepresentationMember {
    /// Position in this revision's ordered membership list.
    pub ordinal: u64,
    /// Ranked view identity.
    pub retrieval_view_id: String,
    /// Existing prepared chunk identity.
    pub chunk_id: String,
    /// Delivery unit represented by that view.
    pub unit_id: String,
    /// Exact model input digest.
    pub input_digest: Digest,
    /// Exact source mapping artifact.
    pub mapping_digest: Digest,
}

/// Immutable members for exactly one recorded graph revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepresentationShard {
    /// Exactly `maestro-representation-shard/1`.
    pub schema_version: String,
    /// Set identity that cannot be replayed into another set.
    pub representation_set_id: String,
    /// Exact source revision covered by these members.
    pub revision_id: String,
    /// Graph digest pinned for this revision.
    pub graph_digest: Digest,
    /// Every ranked membership, in graph order.
    pub members: Vec<RepresentationMember>,
}

/// Set identity and layout supplied when beginning a build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepresentationSetSpec {
    /// Owning collection.
    pub collection_id: String,
    /// Owning chunk set.
    pub chunk_set_id: String,
    /// Immutable representation identity.
    pub id: String,
    /// Complete graph profile digest.
    pub profile_digest: Digest,
    /// Exact representation layout.
    pub layout: RepresentationLayout,
}

/// A scoped representation lookup.
#[derive(Clone, Copy, Debug)]
pub struct RepresentationKey<'a> {
    /// Owning collection.
    pub collection_id: &'a str,
    /// Owning chunk set.
    pub chunk_set_id: &'a str,
    /// Representation identity.
    pub id: &'a str,
}

/// Recorded state of an immutable representation set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationState {
    /// The intended representation has not yet been verified complete.
    Building,
    /// Every graph revision has one validated shard.
    Complete,
    /// Terminal failed build.
    Failed,
}

/// SQL-only identity, layout and lifecycle state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepresentationSet {
    /// Owning collection.
    pub collection_id: String,
    /// Owning chunk set.
    pub chunk_set_id: String,
    /// Immutable representation identity.
    pub id: String,
    /// Complete graph profile digest.
    pub profile_digest: Digest,
    /// Exact representation layout.
    pub layout: RepresentationLayout,
    /// Build lifecycle state.
    pub state: RepresentationState,
}
