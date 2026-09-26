//! A chunk set's identity and its manifest, `maestro-chunk-set/1`: the
//! artifact a complete chunk set pins, which says what it was cut from and
//! what became of each eligible revision.
//!
//! A chunk set's id is `chunk-set-` followed by the SHA-256 of the JSON array
//! of the schema, the collection, the chunker's profile, its preparation
//! profile, the counter's contract ID and the sorted IDs of the eligible
//! revisions: a new profile, counter or set of revisions is a new chunk set.

use super::{
    failure::Error,
    report::{LeftOut, Refusal, Report},
};
use maestro_canonicalization::{CHUNKER_VERSION, PREPARATION_PROFILE};
use maestro_kernel::{artifact::Digest, store::Database};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

/// The manifest's schema.
const SCHEMA: &str = "maestro-chunk-set/1";

/// What a complete chunk set was cut from and what became of each eligible
/// revision: the manifest it pins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    /// Always [`SCHEMA`].
    pub(super) schema: String,
    /// The collection prepared.
    pub(super) collection: String,
    /// The chunk set.
    pub(super) chunk_set: String,
    /// The chunker's profile.
    pub(super) chunk_profile: String,
    /// The chunker's preparation profile, which shapes each prepared input.
    pub(super) preparation_profile: String,
    /// The contract ID of the counter.
    pub(super) counter: String,
    /// The eligible revisions, in ID order.
    pub(super) revisions: Vec<String>,
    /// Each eligible revision left as an exact duplicate, with its group's
    /// smallest revision, which is prepared once for the group, unless it is
    /// refused: then `refusals` names it, and no revision of the group is
    /// prepared.
    pub(super) duplicates: BTreeMap<String, String>,
    /// The groups of near duplicates among the smallest revisions of the
    /// groups of the same content, found before chunking: a member may be
    /// refused.
    pub(super) near_duplicate_groups: Vec<String>,
    /// The revisions refused, in revision order.
    pub(super) refusals: Vec<Refusal>,
    /// The documents left out, as they were when the set was built, in
    /// document order.
    pub(super) left_out: Vec<LeftOut>,
    /// The chunks the set holds.
    pub(super) chunks: u64,
    /// The tokens of their prepared inputs.
    pub(super) tokens: u64,
}

impl Manifest {
    /// The manifest of the chunk set `chunk_set` of `collection`, counted by
    /// `counter`, from the IDs of the eligible `revisions`, sorted, with
    /// nothing prepared yet.
    pub(super) fn new(
        collection: &str,
        chunk_set: &str,
        counter: &str,
        revisions: Vec<String>,
    ) -> Self {
        Self {
            schema: SCHEMA.to_owned(),
            collection: collection.to_owned(),
            chunk_set: chunk_set.to_owned(),
            chunk_profile: CHUNKER_VERSION.to_owned(),
            preparation_profile: PREPARATION_PROFILE.to_owned(),
            counter: counter.to_owned(),
            revisions,
            duplicates: BTreeMap::new(),
            near_duplicate_groups: Vec::new(),
            refusals: Vec::new(),
            left_out: Vec::new(),
            chunks: 0,
            tokens: 0,
        }
    }

    /// The report of the preparation it describes, as it stands.
    pub(super) fn report(&self, prepared: u64) -> Report {
        Report {
            collection: self.collection.clone(),
            chunk_set: self.chunk_set.clone(),
            chunk_profile: self.chunk_profile.clone(),
            counter: self.counter.clone(),
            eligible: count(self.revisions.len()),
            left_out: count(self.left_out.len()),
            prepared,
            duplicates: count(self.duplicates.len()),
            near_duplicate_groups: count(self.near_duplicate_groups.len()),
            chunks: self.chunks,
            tokens: self.tokens,
            refused: count(self.refusals.len()),
            refusals: self.refusals.clone(),
            left_out_documents: self.left_out.clone(),
        }
    }

    /// The report of the complete preparation it describes: every eligible
    /// revision neither a duplicate nor refused was prepared.
    pub(super) fn final_report(&self) -> Report {
        let settled = self.duplicates.len() + self.refusals.len();
        self.report(count(self.revisions.len().saturating_sub(settled)))
    }

    /// Stores the manifest in `database` as an artifact, and returns its
    /// digest.
    pub(super) fn store(&self, database: &Database) -> Result<Digest, Error> {
        let json = serde_json::to_vec_pretty(self).map_err(Error::Manifest)?;
        database
            .put(&json, "application/json")
            .map_err(Error::Artifacts)
    }

    /// The manifest stored as the artifact `digest`.
    pub(super) fn read(database: &Database, digest: &Digest) -> Result<Self, Error> {
        let json = database.get(digest).map_err(Error::Artifacts)?;
        serde_json::from_slice(&json).map_err(Error::Manifest)
    }
}

/// The id of the chunk set of `collection` counted by `counter` from the
/// eligible `revisions`, in ID order.
pub(super) fn id_of(collection: &str, counter: &str, revisions: &[String]) -> String {
    let identity = json!([
        SCHEMA,
        collection,
        CHUNKER_VERSION,
        PREPARATION_PROFILE,
        counter,
        revisions
    ]);
    format!(
        "chunk-set-{}",
        Digest::of(identity.to_string().as_bytes()).as_str()
    )
}

/// `items` as a count.
fn count(items: usize) -> u64 {
    u64::try_from(items).unwrap_or(u64::MAX)
}
