//! The committed schemas of the public events: each the one its type
//! generates, none narrowed by its type, and each followed by the data of
//! its events' envelopes. The ignored test at the end is the command that
//! regenerates the committed files after an additive change.

use super::{
    narrowing::narrowings,
    support::{SCOPE, Scratch, whole},
    validation::violations,
};
use crate::journal::{
    Envelope, GenerationPublished, GenerationRetired, HeldDisposition, ImportCompleted, Machine,
    NewEvent, PUBLIC_EVENTS, RevisionHeld, knowledge::schema_of,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// What every public event's `dataschema` starts with; the name and major
/// version of its schema follow.
const SCHEMAS: &str = "maestro://schemas/events/";

/// The schema of `generation.published`, which the compatibility tests
/// change.
const PUBLISHED: &str = "knowledge.generation.published/1";

/// The stream the tests record their events on.
const STREAM: &str = "collection/demo";

/// The committed file of the schema named `schema`, such as
/// `knowledge.generation.published/1`: `schemas/events/<name>/<major>.json`
/// at the root of the workspace.
fn path_of(schema: &str) -> PathBuf {
    let (name, major) = schema.split_once('/').unwrap();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2);
    let events = workspace.unwrap().join("schemas").join("events");
    events.join(name).join(format!("{major}.json"))
}

/// The committed schema named `schema`.
fn committed(schema: &str) -> Value {
    let path = path_of(schema);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text).unwrap()
}

/// `data` as JSON, as its emitter records it.
fn json(data: impl Serialize) -> Value {
    serde_json::to_value(data).unwrap()
}

/// One event of each public type, with data such as its emitter records.
fn samples() -> [(&'static str, Value); 4] {
    let collection = || "demo".to_owned();
    [
        (
            ImportCompleted::TYPE,
            json(ImportCompleted {
                collection: collection(),
                imported: 7,
                unchanged: 2,
                refused: 1,
            }),
        ),
        (
            RevisionHeld::TYPE,
            json(RevisionHeld {
                collection: collection(),
                revision: "revision-7".to_owned(),
                disposition: HeldDisposition::NeedsReextraction,
            }),
        ),
        (
            GenerationPublished::TYPE,
            json(GenerationPublished {
                collection: collection(),
                generation: 8,
                point_count: 81_234,
            }),
        ),
        (
            GenerationRetired::TYPE,
            json(GenerationRetired {
                collection: collection(),
                generation: 7,
            }),
        ),
    ]
}

#[test]
fn the_four_knowledge_events_are_public_each_with_its_schema() {
    let named: Vec<_> = PUBLIC_EVENTS
        .iter()
        .map(|event| (event.r#type, event.schema))
        .collect();
    assert_eq!(
        named,
        [
            (
                "maestro.knowledge.import.completed.v1",
                "knowledge.import.completed/1"
            ),
            (
                "maestro.knowledge.revision.held.v1",
                "knowledge.revision.held/1"
            ),
            (
                "maestro.knowledge.generation.published.v1",
                "knowledge.generation.published/1"
            ),
            (
                "maestro.knowledge.generation.retired.v1",
                "knowledge.generation.retired/1"
            ),
        ]
    );
    let kinds = samples().map(|(kind, _)| kind);
    assert_eq!(kinds, PUBLIC_EVENTS.map(|event| event.r#type));
}

#[test]
fn each_committed_schema_is_the_one_its_type_generates() {
    for event in PUBLIC_EVENTS {
        assert_eq!(
            committed(event.schema),
            (event.generate)().to_value(),
            "{}: after an additive change, regenerate it with the command of \
             schemas/events/README.md",
            event.schema
        );
    }
}

#[test]
fn no_type_removes_or_narrows_what_its_committed_schema_held() {
    for event in PUBLIC_EVENTS {
        let fresh = (event.generate)().to_value();
        let found = narrowings(&committed(event.schema), &fresh);
        assert_eq!(found, Vec::<String>::new(), "{}", event.schema);
    }
}

#[test]
fn the_envelope_of_each_public_event_carries_data_its_committed_schema_accepts() {
    let scratch = Scratch::new();
    let database = scratch.open();
    for (kind, data) in &samples() {
        let event = NewEvent {
            stream: STREAM,
            r#type: kind,
            subject: STREAM,
            scope: SCOPE,
            data,
        };
        database.record(&event).unwrap();
    }
    let machine = Machine::parse("test-machine").unwrap();
    let envelopes: Vec<Value> = whole(&database, STREAM)
        .into_iter()
        .map(|event| json(Envelope::new(event, &machine)))
        .collect();
    assert_eq!(envelopes.len(), PUBLIC_EVENTS.len());
    for envelope in &envelopes {
        let dataschema = envelope["dataschema"].as_str().unwrap();
        let schema = committed(dataschema.strip_prefix(SCHEMAS).unwrap());
        let found = violations(&envelope["data"], &schema, &schema);
        assert_eq!(found, Vec::<String>::new(), "{envelope}");
    }
}

/// `GenerationPublished` with its point count removed.
#[derive(JsonSchema)]
#[expect(dead_code, reason = "only its schema is generated")]
struct WithoutPointCount {
    /// The collection it is a generation of.
    collection: String,
    /// The generation published.
    generation: u64,
}

/// `GenerationPublished` with an optional field added.
#[derive(JsonSchema)]
#[expect(dead_code, reason = "only its schema is generated")]
struct WithOptionalField {
    /// The collection it is a generation of.
    collection: String,
    /// The generation published.
    generation: u64,
    /// How many points its verification counted.
    point_count: u64,
    /// The chunk set it is built from, when its emitter writes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    chunk_set: Option<String>,
}

/// `GenerationPublished` with a required field added.
#[derive(JsonSchema)]
#[expect(dead_code, reason = "only its schema is generated")]
struct WithRequiredField {
    /// The collection it is a generation of.
    collection: String,
    /// The generation published.
    generation: u64,
    /// How many points its verification counted.
    point_count: u64,
    /// The chunk set it is built from.
    chunk_set: String,
}

#[test]
fn removing_a_field_from_a_type_fails_the_compatibility_test() {
    let fresh = schema_of::<WithoutPointCount>().to_value();
    assert_eq!(
        narrowings(&committed(PUBLISHED), &fresh),
        ["property point_count is gone"]
    );
}

#[test]
fn an_added_optional_field_passes_the_compatibility_test() {
    let fresh = schema_of::<WithOptionalField>().to_value();
    assert_eq!(
        narrowings(&committed(PUBLISHED), &fresh),
        Vec::<String>::new()
    );
}

#[test]
fn a_new_required_field_fails_the_compatibility_test() {
    let fresh = schema_of::<WithRequiredField>().to_value();
    assert_eq!(
        narrowings(&committed(PUBLISHED), &fresh),
        ["property chunk_set is newly required"]
    );
}

#[test]
#[ignore = "writes schemas/events: the command that regenerates the committed schemas"]
fn regenerate_the_committed_schemas_after_an_additive_change() {
    let generated: Vec<(&str, Value)> = PUBLIC_EVENTS
        .iter()
        .map(|event| (event.schema, (event.generate)().to_value()))
        .collect();
    let refused: Vec<String> = generated
        .iter()
        .filter(|(schema, _)| path_of(schema).exists())
        .flat_map(|(schema, fresh)| {
            let found = narrowings(&committed(schema), fresh);
            found
                .into_iter()
                .map(move |line| format!("{schema}: {line}"))
        })
        .collect();
    assert!(
        refused.is_empty(),
        "nothing written: a narrowing needs a new major version\n{}",
        refused.join("\n")
    );
    for (schema, fresh) in &generated {
        let path = path_of(schema);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = serde_json::to_string_pretty(fresh).unwrap();
        fs::write(&path, text + "\n").unwrap();
    }
}
