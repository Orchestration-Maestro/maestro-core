//! The public event schemas may add optional properties within a major, but
//! must preserve every schema released on the exact predecessor revision.

use super::{
    breaking::breaking_changes,
    regeneration::{INDEX, file_of},
    support::Scratch,
};
use crate::journal::knowledge::schema_of;
use schemars::JsonSchema;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

/// The directory of committed schemas in this workspace.
fn schema_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("schemas")
        .join("events")
}

/// Whether `schema` is a safe `<name>/<major>` catalogue key.
fn valid_schema_name(schema: &str) -> bool {
    let Some((name, major)) = schema.split_once('/') else {
        return false;
    };
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !major.is_empty()
        && major != "."
        && major != ".."
        && !major.contains('/')
        && !major.contains('\\')
        && major.bytes().all(|byte| byte.is_ascii_digit())
}

/// The schema files physically present in a catalogue, independent of its
/// index. An unexpected nested directory or symlink makes the baseline
/// unreadable rather than silently hiding a released file.
fn schema_files(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut schemas = BTreeSet::new();
    let entries = fs::read_dir(root).map_err(|error| format!("{}: {error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", root.display()))?;
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", entry.path().display()))?;
        if kind.is_symlink() {
            return Err(format!("{}: unexpected symlink", entry.path().display()));
        }
        if !kind.is_dir() {
            continue;
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| format!("{}: schema name is not UTF-8", entry.path().display()))?;
        if name.is_empty() || name == "." || name == ".." || name.contains('\\') {
            return Err(format!("{}: invalid schema name", entry.path().display()));
        }
        for file in fs::read_dir(entry.path())
            .map_err(|error| format!("{}: {error}", entry.path().display()))?
        {
            let file = file.map_err(|error| format!("{}: {error}", entry.path().display()))?;
            let path = file.path();
            let kind = file
                .file_type()
                .map_err(|error| format!("{}: {error}", path.display()))?;
            if kind.is_dir() || kind.is_symlink() {
                return Err(format!(
                    "{}: unexpected nested schema entry",
                    path.display()
                ));
            }
            if !kind.is_file() || path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let major = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| format!("{}: schema major is not UTF-8", path.display()))?;
            let schema = format!("{name}/{major}");
            if !valid_schema_name(&schema) {
                return Err(format!("{}: invalid schema name", path.display()));
            }
            schemas.insert(schema);
        }
    }
    Ok(schemas)
}

/// The index entries in a catalogue, or `None` when its index is absent.
fn schema_index(root: &Path) -> Result<Option<BTreeSet<String>>, String> {
    let path = root.join(INDEX);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let names: Vec<String> =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut schemas = BTreeSet::new();
    for name in names {
        if !valid_schema_name(&name) {
            return Err(format!("{}: invalid schema name {name:?}", path.display()));
        }
        if !schemas.insert(name.clone()) {
            return Err(format!(
                "{}: duplicate schema name {name:?}",
                path.display()
            ));
        }
    }
    Ok(Some(schemas))
}

/// The parsed schema named `name` in `root`, if its file is present.
fn schema_value(root: &Path, name: &str) -> Result<Option<Value>, String> {
    let path = file_of(root, name);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let value =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(Some(value))
}

/// Whether the candidate preserves every schema file and index entry of the
/// released predecessor, using that predecessor's files rather than the
/// candidate's event catalogue to discover old names.
fn released_schema_changes(released: &Path, candidate: &Path) -> Result<(), Vec<String>> {
    let released_files = schema_files(released).map_err(|error| vec![error])?;
    let candidate_files = schema_files(candidate).map_err(|error| vec![error])?;
    let released_index = schema_index(released).map_err(|error| vec![error])?;
    let released_index = match released_index {
        Some(index) => index,
        None if released_files.is_empty() => BTreeSet::new(),
        None => {
            return Err(vec![format!(
                "{}: index.json is missing",
                released.display()
            )]);
        }
    };
    let candidate_index = schema_index(candidate)
        .map_err(|error| vec![error])?
        .ok_or_else(|| vec![format!("{}: index.json is missing", candidate.display())])?;
    let released_names: BTreeSet<_> = released_files.union(&released_index).cloned().collect();
    let mut changes = Vec::new();
    for name in released_names {
        let Some(before) = schema_value(released, &name).map_err(|error| vec![error])? else {
            changes.push(format!("{name}: released index has no schema file"));
            continue;
        };
        if !candidate_index.contains(&name) {
            changes.push(format!("{name}: missing from the current schema index"));
        }
        if !candidate_files.contains(&name) {
            changes.push(format!("{name}: released schema file is missing"));
            continue;
        }
        let Some(after) = schema_value(candidate, &name).map_err(|error| vec![error])? else {
            changes.push(format!("{name}: released schema file is unreadable"));
            continue;
        };
        changes.extend(
            breaking_changes(&before, &after)
                .into_iter()
                .map(|change| format!("{name}: {change}")),
        );
    }
    if changes.is_empty() {
        Ok(())
    } else {
        Err(changes)
    }
}

/// The event type as it was released before its `answer` field was removed.
#[derive(JsonSchema)]
#[expect(dead_code, reason = "synthetic released event schema")]
struct ReleasedAnswer {
    /// The required answer in the released event.
    answer: String,
}

/// The updated event type after removing the released `answer` field.
#[derive(JsonSchema)]
#[expect(dead_code, reason = "synthetic current event schema")]
struct CurrentAnswer {
    /// The replacement field emitted by the updated event type.
    status: String,
}

/// Writes a small synthetic catalogue to exercise the predecessor check.
fn write_catalogue(root: &Path, schemas: &[(&str, Value)]) {
    fs::create_dir_all(root).unwrap();
    let names: Vec<_> = schemas.iter().map(|(name, _)| *name).collect();
    fs::write(root.join(INDEX), serde_json::to_vec(&names).unwrap()).unwrap();
    for (name, schema) in schemas {
        let path = file_of(root, name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, serde_json::to_vec(schema).unwrap()).unwrap();
    }
}

#[test]
fn a_field_deletion_is_refused_when_the_candidate_type_and_schema_agree() {
    let released = Scratch::new();
    let candidate = Scratch::new();
    write_catalogue(
        &released.0,
        &[(
            "knowledge.answer/1",
            schema_of::<ReleasedAnswer>().to_value(),
        )],
    );
    write_catalogue(
        &candidate.0,
        &[(
            "knowledge.answer/1",
            schema_of::<CurrentAnswer>().to_value(),
        )],
    );

    let found = released_schema_changes(&released.0, &candidate.0).unwrap_err();
    assert!(found.contains(&"knowledge.answer/1: property answer is gone".to_owned()));
}

#[test]
fn a_released_schema_cannot_be_deleted_with_its_index_entry() {
    let released = Scratch::new();
    let candidate = Scratch::new();
    write_catalogue(
        &released.0,
        &[("knowledge.answer/1", json!({"type": "string"}))],
    );
    write_catalogue(
        &candidate.0,
        &[("knowledge.other/1", json!({"type": "string"}))],
    );

    assert!(released_schema_changes(&released.0, &candidate.0).is_err());
}

#[test]
fn a_released_schema_rejects_type_enum_bound_and_requiredness_changes() {
    let released = Scratch::new();
    let candidate = Scratch::new();
    let old = json!({
        "type": "object",
        "properties": {
            "count": {"type": "integer", "minimum": 0},
            "kind": {"type": "string", "enum": ["a", "b"]},
            "label": {"type": "string"}
        },
        "required": ["count", "kind"]
    });
    let changes = [
        json!({
            "type": "object",
            "properties": {
                "count": {"type": "string", "minimum": 0},
                "kind": {"type": "string", "enum": ["a", "b"]},
                "label": {"type": "string"}
            },
            "required": ["count", "kind"]
        }),
        json!({
            "type": "object",
            "properties": {
                "count": {"type": "integer", "minimum": 0},
                "kind": {"type": "string", "enum": ["a"]},
                "label": {"type": "string"}
            },
            "required": ["count", "kind"]
        }),
        json!({
            "type": "object",
            "properties": {
                "count": {"type": "integer", "minimum": 1},
                "kind": {"type": "string", "enum": ["a", "b"]},
                "label": {"type": "string"}
            },
            "required": ["count", "kind"]
        }),
        json!({
            "type": "object",
            "properties": {
                "count": {"type": "integer", "minimum": 0},
                "kind": {"type": "string", "enum": ["a", "b"]},
                "label": {"type": "string"}
            },
            "required": ["count", "kind", "label"]
        }),
    ];
    write_catalogue(&released.0, &[("knowledge.answer/1", old)]);

    for changed in changes {
        write_catalogue(&candidate.0, &[("knowledge.answer/1", changed)]);
        assert!(released_schema_changes(&released.0, &candidate.0).is_err());
    }
}

#[test]
fn a_released_schema_allows_an_optional_property_and_a_new_major() {
    let released = Scratch::new();
    let candidate = Scratch::new();
    let old = json!({
        "type": "object",
        "properties": {"answer": {"type": "string"}},
        "required": ["answer"]
    });
    let extended = json!({
        "type": "object",
        "properties": {
            "answer": {"type": "string"},
            "extra": {"type": "string"}
        },
        "required": ["answer"]
    });
    write_catalogue(&released.0, &[("knowledge.answer/1", old)]);
    write_catalogue(
        &candidate.0,
        &[
            ("knowledge.answer/1", extended),
            ("knowledge.answer/2", json!({"type": "string"})),
        ],
    );

    assert_eq!(released_schema_changes(&released.0, &candidate.0), Ok(()));
}

#[test]
fn an_empty_predecessor_is_a_valid_first_release() {
    let released = Scratch::new();
    let candidate = Scratch::new();
    write_catalogue(
        &candidate.0,
        &[("knowledge.answer/1", json!({"type": "string"}))],
    );

    assert_eq!(released_schema_changes(&released.0, &candidate.0), Ok(()));
}

#[test]
fn an_unreadable_predecessor_is_not_treated_as_a_first_release() {
    let scratch = Scratch::new();
    let missing = scratch.0.join("missing");
    let candidate = Scratch::new();
    write_catalogue(
        &candidate.0,
        &[("knowledge.answer/1", json!({"type": "string"}))],
    );

    assert!(released_schema_changes(&missing, &candidate.0).is_err());
}

#[test]
#[ignore = "requires the exact predecessor catalogue in MAESTRO_EVENT_SCHEMAS_BASE"]
fn public_event_schemas_preserve_the_released_predecessor() {
    let base = env::var_os("MAESTRO_EVENT_SCHEMAS_BASE")
        .expect("the workflow must provide MAESTRO_EVENT_SCHEMAS_BASE");
    assert_eq!(
        released_schema_changes(&PathBuf::from(base), &schema_root()),
        Ok(())
    );
}
