//! Bounded, strict parsing into checked values. The caller has already
//! bounded each file's bytes. YAML frontmatter is read node by node within
//! the depth limit and a node budget; a TOML document, which has no aliases,
//! is parsed into its generic tree (its parser caps its own recursion) and
//! its depth measured. Only then are its keys and types checked against a
//! descriptor. Duplicate keys are refused while parsing.

use super::{
    descriptor::{Field, FieldType},
    secrets::SecretReference,
    types::{Float, Problems, Value},
    yaml::{self, Node},
};
use crate::limits::Limits;
use std::collections::{BTreeMap, BTreeSet};
use toml::de::Error as TomlError;

/// A document's top level: its keys and values.
pub(super) type Table = BTreeMap<String, Value>;

pub(super) use super::types::is_name;

/// Whether `name` is a tool name: lower-case ASCII letters, digits, `_` and
/// `-`, nonempty.
pub(super) fn is_tool(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}

/// The container levels of a TOML value, the root counting as one.
fn toml_depth(value: &toml::Value) -> usize {
    match value {
        toml::Value::Array(items) => 1 + items.iter().map(toml_depth).max().unwrap_or(0),
        toml::Value::Table(table) => 1 + table.values().map(toml_depth).max().unwrap_or(0),
        _ => 0,
    }
}

/// The message refusing a document deeper than `limits` allow.
fn too_deep(limits: &Limits) -> String {
    format!("deeper than {} levels", limits.source_depth)
}

/// Joins `key` below `parent`.
fn below(parent: &str, key: &str) -> String {
    if parent.is_empty() {
        key.to_owned()
    } else {
        format!("{parent}.{key}")
    }
}

/// The checked [`Value`]s of the YAML mapping `entries` at `key`, or the key
/// and message refusing them.
fn from_map(entries: Vec<(Node, Node)>, key: &str) -> Result<Table, (String, String)> {
    let mut table = Table::new();
    for (name, value) in entries {
        let Node::Text(name) = name else {
            return Err((key.to_owned(), "has a key that is not a string".to_owned()));
        };
        let value = from_node(value, &below(key, &name))?;
        table.insert(name, value);
    }
    Ok(table)
}

/// A YAML node as a checked [`Value`], or the key and message refusing it.
fn from_node(node: Node, key: &str) -> Result<Value, (String, String)> {
    let refuse = |message: &str| Err((key.to_owned(), message.to_owned()));
    match node {
        Node::Text(text) => Ok(Value::Text(text)),
        Node::Boolean(flag) => Ok(Value::Boolean(flag)),
        Node::Integer(integer) => Ok(Value::Integer(integer)),
        Node::Huge => refuse("must fit a 64-bit signed integer"),
        Node::Float(number) => finite(number, key),
        Node::List(items) => items
            .into_iter()
            .map(|item| from_node(item, key))
            .collect::<Result<_, _>>()
            .map(Value::List),
        Node::Map(entries) => from_map(entries, key).map(Value::Table),
        Node::Null => refuse("must not be empty"),
        Node::Tagged => refuse("must not carry a YAML tag"),
    }
}

/// The fraction `number` at `key`, if it is finite.
fn finite(number: f64, key: &str) -> Result<Value, (String, String)> {
    Float::new(number)
        .map(Value::Float)
        .ok_or_else(|| (key.to_owned(), "must be a finite number".to_owned()))
}

/// A TOML value as a checked [`Value`], or the key and message refusing it.
fn from_toml(value: toml::Value, key: &str) -> Result<Value, (String, String)> {
    match value {
        toml::Value::String(text) => Ok(Value::Text(text)),
        toml::Value::Integer(integer) => Ok(Value::Integer(integer)),
        toml::Value::Float(number) => finite(number, key),
        toml::Value::Boolean(flag) => Ok(Value::Boolean(flag)),
        toml::Value::Array(items) => items
            .into_iter()
            .map(|item| from_toml(item, key))
            .collect::<Result<_, _>>()
            .map(Value::List),
        toml::Value::Table(entries) => entries
            .into_iter()
            .map(|(name, value)| {
                let value = from_toml(value, &below(key, &name))?;
                Ok((name, value))
            })
            .collect::<Result<_, _>>()
            .map(Value::Table),
        toml::Value::Datetime(_) => Err((
            key.to_owned(),
            "must be a string, number, boolean, list or table".to_owned(),
        )),
    }
}

/// The top-level table of YAML frontmatter, read within the depth limit and
/// a budget of twice its bytes in nodes.
///
/// # Errors
///
/// The key, empty for the whole document, and message refusing it.
pub(crate) fn yaml_table(yaml: &str, limits: &Limits) -> Result<Table, (String, String)> {
    let whole = |message: String| (String::new(), message);
    let budget = yaml.len().saturating_mul(2);
    match yaml::read(yaml, limits.source_depth, budget).map_err(whole)? {
        Node::Map(entries) => from_map(entries, ""),
        _ => Err(whole("frontmatter must be a mapping".to_owned())),
    }
}

/// The one-line message of the TOML error `error` in `text`: where it is and
/// what is wrong, never the source it quotes.
fn toml_error(text: &str, error: &TomlError) -> String {
    let offset = error.span().map_or(0, |span| span.start);
    let before = text.get(..offset).unwrap_or(text);
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map_or(0, |last| last.chars().count())
        + 1;
    let message: Vec<&str> = error.message().split_whitespace().collect();
    format!(
        "invalid TOML at line {line}, column {column}: {}",
        message.join(" ")
    )
}

/// The top-level table of a TOML document, after bounding its depth.
///
/// # Errors
///
/// The key, empty for the whole document, and message refusing it.
pub(super) fn toml_table(text: &str, limits: &Limits) -> Result<Table, (String, String)> {
    let tree: toml::Table =
        toml::from_str(text).map_err(|error| (String::new(), toml_error(text, &error)))?;
    let tree = toml::Value::Table(tree);
    if toml_depth(&tree) > limits.source_depth {
        return Err((String::new(), too_deep(limits)));
    }
    match from_toml(tree, "")? {
        Value::Table(table) => Ok(table),
        _ => Err((String::new(), "not a TOML table".to_owned())),
    }
}

/// Notes each item `items` holds twice, under `key`.
pub(super) fn once<'a>(
    items: impl IntoIterator<Item = &'a str>,
    key: &str,
    problems: &mut Problems,
) {
    let mut seen = BTreeSet::new();
    for item in items {
        if !seen.insert(item) {
            problems.push((key.to_owned(), format!("lists {item:?} twice")));
        }
    }
}

/// The message refusing `value` as the scalar field type `kind`, if it
/// does not fit.
fn scalar_problem(kind: &FieldType, value: &Value) -> Option<&'static str> {
    let (fits, message) = match kind {
        FieldType::Text => (
            value.text().is_some_and(|text| !text.trim().is_empty()),
            "must be a nonempty string",
        ),
        FieldType::Integer => (matches!(value, Value::Integer(_)), "must be an integer"),
        FieldType::Number => (
            matches!(value, Value::Integer(_) | Value::Float(_)),
            "must be a number",
        ),
        _ => (matches!(value, Value::Boolean(_)), "must be a boolean"),
    };
    (!fits).then_some(message)
}

/// Notes what is wrong with `value`, the field `key` of type `kind`.
fn field_problems(key: &str, kind: &FieldType, value: &Value, problems: &mut Problems) {
    match kind {
        FieldType::TextList | FieldType::TextSequence | FieldType::ToolList => {
            list_problems(key, kind, value, problems);
        }
        FieldType::ScalarTable | FieldType::ListTable => {
            table_problems(key, kind, value, problems);
        }
        FieldType::Table { fields: inner } => match value {
            Value::Table(entries) => fields(entries, inner, key, problems),
            _ => problems.push((key.to_owned(), "must be a table".to_owned())),
        },
        FieldType::SecretReference => {
            if value.decode::<SecretReference>().is_err() {
                problems.push((
                    key.to_owned(),
                    "must be exactly one environment-variable or keychain reference".to_owned(),
                ));
            }
        }
        FieldType::Delegated { .. } => {}
        FieldType::Text | FieldType::Integer | FieldType::Number | FieldType::Boolean => {
            if let Some(message) = scalar_problem(kind, value) {
                problems.push((key.to_owned(), message.to_owned()));
            }
        }
    }
}

/// Notes what is wrong with the list `value`, the field `key`.
fn list_problems(key: &str, kind: &FieldType, value: &Value, problems: &mut Problems) {
    let Some(items) = value.texts() else {
        problems.push((key.to_owned(), "must be a list of strings".to_owned()));
        return;
    };
    for item in &items {
        if item.trim().is_empty() {
            problems.push((key.to_owned(), "must not list an empty string".to_owned()));
        } else if *kind == FieldType::ToolList && !is_tool(item) {
            problems.push((key.to_owned(), format!("{item:?} is not a tool name")));
        }
    }
    if *kind != FieldType::TextSequence {
        once(items, key, problems);
    }
}

/// Notes what is wrong with the table `value`, the field `key`.
fn table_problems(key: &str, kind: &FieldType, value: &Value, problems: &mut Problems) {
    let Value::Table(entries) = value else {
        problems.push((key.to_owned(), "must be a table".to_owned()));
        return;
    };
    for (name, entry) in entries {
        let fits = match kind {
            FieldType::ListTable => entry.texts().is_some(),
            _ => matches!(
                entry,
                Value::Text(_) | Value::Integer(_) | Value::Float(_) | Value::Boolean(_)
            ),
        };
        if !fits {
            let message = match kind {
                FieldType::ListTable => "must be a list of strings",
                _ => "must be a string, number or boolean",
            };
            problems.push((below(key, name), message.to_owned()));
        }
    }
}

/// Notes what is wrong with `table` under `prefix` against `fields`: each
/// key must be a field, of its type, and each required field present.
pub(super) fn fields(table: &Table, fields: &[Field], prefix: &str, problems: &mut Problems) {
    for (key, value) in table {
        match fields.iter().find(|field| field.key == *key) {
            Some(field) => field_problems(&below(prefix, key), &field.kind, value, problems),
            None => problems.push((below(prefix, key), "unknown key".to_owned())),
        }
    }
    for field in fields
        .iter()
        .filter(|field| field.required && !table.contains_key(&field.key))
    {
        problems.push((below(prefix, &field.key), "missing".to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::table_problems;
    use crate::source::{FieldType, Value};
    use std::collections::BTreeMap;

    #[test]
    fn list_table_accepts_lists_and_explains_scalar_refusals() {
        let mut problems = vec![];
        let value = Value::Table(BTreeMap::from([(
            "good".to_owned(),
            Value::List(vec![Value::Text("text".to_owned())]),
        )]));
        table_problems("bindings", &FieldType::ListTable, &value, &mut problems);
        assert!(problems.is_empty());
        let value = Value::Table(BTreeMap::from([(
            "bad".to_owned(),
            Value::Text("text".to_owned()),
        )]));
        table_problems("bindings", &FieldType::ListTable, &value, &mut problems);
        assert_eq!(
            problems,
            [(
                "bindings.bad".to_owned(),
                "must be a list of strings".to_owned()
            )]
        );
    }
}
