//! The command that regenerates the committed schemas of the public events,
//! tested on a scratch directory: it writes each schema its type generates
//! and the index that lists them, and writes nothing when a type breaks its
//! committed schema, a schema the index lists has lost its file, or the
//! index lists a schema no public event has.
//!
//! It guards the documented command only. Deleting a schema and editing the
//! index together still get past it: the full guard is a later CI step that
//! compares each schema with the one committed on `main`, its released
//! predecessor (FR-S1-008b).

use super::{breaking::breaking_changes, support::Scratch};
use crate::journal::{GenerationPublished, GenerationRetired, PublicEvent, knowledge::schema_of};
use schemars::{JsonSchema, Schema};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

/// The file of a directory of schemas that lists the committed ones, by
/// name, in order.
pub(super) const INDEX: &str = "index.json";

/// The file of the schema named `schema`, such as
/// `knowledge.generation.published/1`, in the directory of schemas `root`:
/// `<name>/<major>.json`.
pub(super) fn file_of(root: &Path, schema: &str) -> PathBuf {
    let (name, major) = schema.split_once('/').unwrap();
    root.join(name).join(format!("{major}.json"))
}

/// Writes into `root` the schema of each of `events` and the index that
/// lists them. Writes nothing, and returns why, one line each, when a type
/// breaks its committed schema, a schema the index lists has lost its file,
/// or the index lists a schema none of `events` has.
pub(super) fn regenerate(events: &[PublicEvent], root: &Path) -> Result<(), Vec<String>> {
    let listed: Vec<String> = read(&root.join(INDEX))
        .map_or_else(Vec::new, |index| serde_json::from_value(index).unwrap());
    let mut refused = Vec::new();
    let mut fresh = Vec::new();
    for event in events {
        let schema = (event.generate)().to_value();
        let name = event.schema;
        match read(&file_of(root, name)) {
            Some(committed) => refused.extend(
                breaking_changes(&committed, &schema)
                    .into_iter()
                    .map(|line| format!("{name}: {line}")),
            ),
            None if listed.iter().any(|listed| listed == name) => refused.push(format!(
                "{name}: {INDEX} lists it and its file is gone: restore it, since a committed \
                 schema is never deleted"
            )),
            None => {}
        }
        fresh.push((name, schema));
    }
    let names: BTreeSet<&str> = events.iter().map(|event| event.schema).collect();
    for name in listed
        .iter()
        .filter(|listed| !names.contains(listed.as_str()))
    {
        refused.push(format!(
            "{name}: {INDEX} lists it and no public event has it: a committed schema is never \
             dropped"
        ));
    }
    if !refused.is_empty() {
        return Err(refused);
    }
    for (name, schema) in &fresh {
        write(&file_of(root, name), schema);
    }
    write(&root.join(INDEX), &Value::from_iter(names));
    Ok(())
}

/// Writes `value` into the file `path`, as pretty JSON and a line break, and
/// the directories it needs.
fn write(path: &Path, value: &Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_string_pretty(value).unwrap() + "\n").unwrap();
}

/// The name of the schema of `generation.published`.
const PUBLISHED: &str = "knowledge.generation.published/1";

/// `GenerationPublished` with its point count removed.
#[derive(JsonSchema)]
#[expect(dead_code, reason = "only its schema is generated")]
struct Narrowed {
    /// The collection it is a generation of.
    collection: String,
    /// The generation published.
    generation: u64,
}

/// `GenerationPublished` with an optional field added.
#[derive(JsonSchema)]
#[expect(dead_code, reason = "only its schema is generated")]
struct Grown {
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

/// `generation.published` with the schema `generate` gives.
fn published(generate: fn() -> Schema) -> PublicEvent {
    PublicEvent {
        r#type: GenerationPublished::TYPE,
        schema: PUBLISHED,
        generate,
    }
}

/// `generation.retired`, as the catalogue has it.
fn retired() -> PublicEvent {
    PublicEvent {
        r#type: GenerationRetired::TYPE,
        schema: "knowledge.generation.retired/1",
        generate: schema_of::<GenerationRetired>,
    }
}

/// The JSON file `path` holds, if it exists.
fn read(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    Some(serde_json::from_str(&text).unwrap())
}

/// A scratch directory of schemas where `generation.published` is committed
/// as the catalogue has it.
fn committed() -> Scratch {
    let scratch = Scratch::new();
    let written = regenerate(&[published(schema_of::<GenerationPublished>)], &scratch.0);
    assert_eq!(written, Ok(()));
    scratch
}

#[test]
fn regenerate_writes_each_schema_and_the_index_into_an_empty_directory() {
    let scratch = Scratch::new();
    let events = [retired(), published(schema_of::<GenerationPublished>)];
    assert_eq!(regenerate(&events, &scratch.0), Ok(()));
    for event in &events {
        let written = read(&file_of(&scratch.0, event.schema));
        assert_eq!(
            written,
            Some((event.generate)().to_value()),
            "{}",
            event.schema
        );
    }
    let index = read(&scratch.0.join(INDEX));
    let names = json!([PUBLISHED, "knowledge.generation.retired/1"]);
    assert_eq!(index, Some(names));
    let text = fs::read_to_string(scratch.0.join(INDEX)).unwrap();
    assert!(text.ends_with("]\n"), "{text:?}");
}

#[test]
fn regenerate_rewrites_a_schema_whose_type_only_grew() {
    let scratch = committed();
    let index = read(&scratch.0.join(INDEX));
    assert_eq!(
        regenerate(&[published(schema_of::<Grown>)], &scratch.0),
        Ok(())
    );
    let written = read(&file_of(&scratch.0, PUBLISHED));
    assert_eq!(written, Some(schema_of::<Grown>().to_value()));
    assert_eq!(read(&scratch.0.join(INDEX)), index);
}

#[test]
fn regenerate_writes_nothing_when_a_type_breaks_its_committed_schema() {
    let scratch = committed();
    let file = file_of(&scratch.0, PUBLISHED);
    let before = (read(&file), read(&scratch.0.join(INDEX)));
    let events = [published(schema_of::<Narrowed>), retired()];
    assert_eq!(
        regenerate(&events, &scratch.0),
        Err(vec![format!("{PUBLISHED}: property point_count is gone")])
    );
    assert_eq!((read(&file), read(&scratch.0.join(INDEX))), before);
    let new = file_of(&scratch.0, "knowledge.generation.retired/1");
    assert!(!new.exists(), "{}", new.display());
}

#[test]
fn regenerate_refuses_a_listed_schema_whose_file_or_event_is_gone() {
    let scratch = committed();
    // `git rm` removes the directory a schema leaves empty.
    fs::remove_dir_all(file_of(&scratch.0, PUBLISHED).parent().unwrap()).unwrap();
    assert_eq!(
        regenerate(&[published(schema_of::<Narrowed>)], &scratch.0),
        Err(vec![format!(
            "{PUBLISHED}: {INDEX} lists it and its file is gone: restore it, since a committed \
             schema is never deleted"
        )])
    );
    assert_eq!(read(&file_of(&scratch.0, PUBLISHED)), None);
    assert_eq!(
        regenerate(&[retired()], &scratch.0),
        Err(vec![format!(
            "{PUBLISHED}: {INDEX} lists it and no public event has it: a committed schema is \
             never dropped"
        )])
    );
    assert_eq!(
        read(&file_of(&scratch.0, "knowledge.generation.retired/1")),
        None
    );
}
