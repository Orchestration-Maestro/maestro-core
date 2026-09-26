//! The public knowledge events (docs/architecture/07 §3.2; FR-S1-008b): the
//! data each carries, as its emitter records it in the journal and a consumer
//! reads it in the envelope, and the catalogue of the public events, which
//! names the schema each one's data follows.
//!
//! Each type lives in the kernel, so that a kernel write records its event in
//! the same transaction as its change. Its schema is generated from the type
//! and committed under `schemas/events/`, and a test fails when the type
//! removes or narrows anything the committed schema held: within a major
//! version an event only gains optional fields, and
//! `schemas/events/README.md` gives the command that regenerates the
//! committed files after such an addition. A change that removes or narrows
//! is a new major version: a new type, `.v2`, beside the old one.

use schemars::{JsonSchema, Schema, generate::SchemaSettings};
use serde::{Deserialize, Serialize};

/// A public event of the catalogue: one that may leave the kernel for a
/// consumer outside it, with the schema its data follows.
#[derive(Debug, Clone, Copy)]
pub struct PublicEvent {
    /// Its type, such as `maestro.knowledge.generation.published.v1`.
    pub r#type: &'static str,
    /// The name and major version of the schema its data follows, such as
    /// `knowledge.generation.published/1`: its `dataschema` is
    /// `maestro://schemas/events/` followed by them, and its committed file
    /// `schemas/events/<name>/<major>.json`.
    pub schema: &'static str,
    /// Generates that schema from the type of its data.
    pub generate: fn() -> Schema,
}

/// The catalogue of the public events, which S1 starts with the four
/// knowledge events.
pub const PUBLIC_EVENTS: [PublicEvent; 4] = [
    PublicEvent {
        r#type: ImportCompleted::TYPE,
        schema: "knowledge.import.completed/1",
        generate: schema_of::<ImportCompleted>,
    },
    PublicEvent {
        r#type: RevisionHeld::TYPE,
        schema: "knowledge.revision.held/1",
        generate: schema_of::<RevisionHeld>,
    },
    PublicEvent {
        r#type: GenerationPublished::TYPE,
        schema: "knowledge.generation.published/1",
        generate: schema_of::<GenerationPublished>,
    },
    PublicEvent {
        r#type: GenerationRetired::TYPE,
        schema: "knowledge.generation.retired/1",
        generate: schema_of::<GenerationRetired>,
    },
];

/// The JSON Schema, draft 2020-12, of the data of type `T` as the kernel
/// serializes it: a field it always writes is required.
pub(super) fn schema_of<T: JsonSchema>() -> Schema {
    SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<T>()
}

/// An import of a collection's corpus manifest completed: what it did with its entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ImportCompleted {
    /// The collection imported.
    pub collection: String,
    /// How many entries it recorded as new revisions.
    pub imported: u64,
    /// How many entries it found recorded already, as they are.
    pub unchanged: u64,
    /// How many entries it refused, each with its reason in the import's report.
    pub refused: u64,
}

impl ImportCompleted {
    /// The type of its events.
    pub const TYPE: &'static str = "maestro.knowledge.import.completed.v1";
}

/// The quality gate held a revision back: it is not indexed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RevisionHeld {
    /// The collection of the revision.
    pub collection: String,
    /// The revision held.
    pub revision: String,
    /// Why it is held: the gate's disposition of it.
    pub disposition: HeldDisposition,
}

impl RevisionHeld {
    /// The type of its events.
    pub const TYPE: &'static str = "maestro.knowledge.revision.held.v1";
}

/// A disposition of the quality gate that holds a revision back from indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HeldDisposition {
    /// Its structure was lost or garbled: it waits for another extraction.
    NeedsReextraction,
    /// It needs a review: a suspected secret, hostile content or unresolved provenance.
    Quarantined,
    /// It is out of scope by a recorded decision.
    Excluded,
}

/// A generation of a collection was published: searches read it from now on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GenerationPublished {
    /// The collection it is a generation of.
    pub collection: String,
    /// The generation published, by its id, which its projection is named after.
    pub generation: u64,
    /// How many points its verification counted.
    pub point_count: u64,
}

impl GenerationPublished {
    /// The type of its events.
    pub const TYPE: &'static str = "maestro.knowledge.generation.published.v1";
}

/// A published generation was retired: no search reads it, and it is kept for rollback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GenerationRetired {
    /// The collection it is a generation of.
    pub collection: String,
    /// The generation retired, by its id.
    pub generation: u64,
}

impl GenerationRetired {
    /// The type of its events.
    pub const TYPE: &'static str = "maestro.knowledge.generation.retired.v1";
}
