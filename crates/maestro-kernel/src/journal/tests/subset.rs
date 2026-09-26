//! What the compatibility check and the validator read of a schema: the
//! subset of JSON Schema 2020-12 that schemars generates for the event types,
//! each `$ref` followed into the schema's own definitions.

use serde_json::{Map, Value};
use std::{collections::BTreeSet, fmt::Display};

/// The keywords that describe a schema without constraining its data.
pub(super) const ANNOTATIONS: [&str; 11] = [
    "$schema",
    "$id",
    "$comment",
    "$defs",
    "title",
    "description",
    "default",
    "examples",
    "deprecated",
    "readOnly",
    "writeOnly",
];

/// The keywords both read. Any other keyword that constrains data is
/// refused: by the check when it changes, by the validator always.
pub(super) const READ: [&str; 13] = [
    "$ref",
    "type",
    "enum",
    "const",
    "oneOf",
    "anyOf",
    "format",
    "minimum",
    "maximum",
    "properties",
    "required",
    "additionalProperties",
    "items",
];

/// How deep both follow nested schemas before they refuse: far deeper than
/// the event types nest.
pub(super) const DEPTH: usize = 32;

/// A bound a schema may set on a number.
pub(super) struct Bound {
    /// Its keyword.
    pub(super) keyword: &'static str,
    /// Whether a value is outside a bound of that value.
    pub(super) outside: fn(f64, f64) -> bool,
    /// Where a value outside it lies.
    pub(super) side: &'static str,
    /// How it moves when a schema tightens it.
    pub(super) tightened: &'static str,
}

/// The bounds both read: `minimum` and `maximum`, each inclusive.
pub(super) const BOUNDS: [Bound; 2] = [
    Bound {
        keyword: "minimum",
        outside: |value, bound| value < bound,
        side: "below",
        tightened: "raised",
    },
    Bound {
        keyword: "maximum",
        outside: |value, bound| value > bound,
        side: "above",
        tightened: "lowered",
    },
];

/// `schema` as one object, with the definition each of its `$ref`s names in
/// `root` merged in, and the references it followed, each once. A `$ref`
/// still there names no definition of `root`.
pub(super) fn resolved(schema: &Value, root: &Value) -> (Map<String, Value>, Vec<String>) {
    let mut merged = schema.as_object().cloned().unwrap_or_default();
    let mut followed = Vec::new();
    while let Some(Value::String(reference)) = merged.get("$ref").cloned() {
        let target = reference
            .strip_prefix('#')
            .and_then(|pointer| root.pointer(pointer))
            .and_then(Value::as_object)
            .filter(|_| !followed.contains(&reference));
        let Some(target) = target else {
            break;
        };
        merged.remove("$ref");
        for (key, value) in target {
            merged.entry(key.clone()).or_insert_with(|| value.clone());
        }
        followed.push(reference);
    }
    (merged, followed)
}

/// Whether `key` is a keyword neither reads that constrains data.
pub(super) fn unread(key: &str) -> bool {
    !ANNOTATIONS.contains(&key) && !READ.contains(&key)
}

/// The JSON types `schema` names in `type`, if it names any.
pub(super) fn types(schema: &Map<String, Value>) -> Option<Vec<String>> {
    match schema.get("type")? {
        Value::String(kind) => Some(vec![kind.clone()]),
        Value::Array(kinds) => Some(
            kinds
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
        ),
        _ => None,
    }
}

/// Whether `types` accept a value of the JSON type `kind`: a `number`
/// accepts an `integer`.
pub(super) fn accepts(types: &[String], kind: &str) -> bool {
    let named = |name: &str| types.iter().any(|kind| kind == name);
    named(kind) || (kind == "integer" && named("number"))
}

/// The JSON type of `value`, as `type` names it.
pub(super) fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// The values `schema` allows, when it lists them: in `enum`, as a `const`,
/// or as a `oneOf` or an `anyOf` whose every branch is a `const`, as
/// schemars writes an enum whose variants are documented.
pub(super) fn allowed_values(schema: &Map<String, Value>) -> Option<Vec<Value>> {
    if let Some(Value::Array(values)) = schema.get("enum") {
        return Some(values.clone());
    }
    if let Some(value) = schema.get("const") {
        return Some(vec![value.clone()]);
    }
    ["oneOf", "anyOf"]
        .into_iter()
        .find_map(|key| constants(schema.get(key)?))
}

/// The constant of each of `branches`, when every one is a `const`.
pub(super) fn constants(branches: &Value) -> Option<Vec<Value>> {
    branches
        .as_array()?
        .iter()
        .map(|branch| branch.get("const").cloned())
        .collect()
}

/// The object `schema` holds under `key`, such as its properties: empty when
/// it holds none.
pub(super) fn members(schema: &Map<String, Value>, key: &str) -> Map<String, Value> {
    schema
        .get(key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

/// The names `schema` lists under `key`, such as its required properties.
pub(super) fn names(schema: &Map<String, Value>, key: &str) -> BTreeSet<String> {
    schema
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// The number `schema` holds under `key`, such as its minimum.
pub(super) fn number(schema: &Map<String, Value>, key: &str) -> Option<f64> {
    schema.get(key)?.as_f64()
}

/// Where a schema nested in the schema at `at` is: `at`, a slash and `step`.
pub(super) fn below(at: &str, step: impl Display) -> String {
    format!("{at}/{step}")
}

/// A finding about the schema or the value at `at`, which it names unless
/// it is the root.
pub(super) fn located(at: &str, message: &str) -> String {
    if at.is_empty() {
        message.to_owned()
    } else {
        format!("{at}: {message}")
    }
}
