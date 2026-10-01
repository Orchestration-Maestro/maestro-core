//! The Maestro metadata every resource declares, read the same way wherever
//! its kind keeps it: schema, owner, stage, 08 rows, workflows, typed
//! references and an optional version.

use super::{
    descriptor::MetadataPlace,
    parse::{Table, is_name, once},
    types::{KnownRows, Maturity, Metadata, Problems, ResourceId, SCHEMA, Value},
};

/// The metadata keys, each a string but the lists `rows`, `workflows` and
/// `requires`.
const KEYS: [&str; 7] = [
    "schema",
    "owner",
    "maturity",
    "rows",
    "workflows",
    "requires",
    "version",
];

/// The keys a string map may not hold without Maestro's prefix: a reader
/// would take them for Maestro's. `version`, which the Agent Skills
/// specification's own example uses, is not among them.
const AMBIGUOUS: [&str; 7] = [
    "schema",
    "owner",
    "maturity",
    "stage",
    "rows",
    "workflows",
    "requires",
];

/// The list-valued metadata keys.
const LISTS: [&str; 3] = ["rows", "workflows", "requires"];

/// What checking metadata needs besides the table.
#[derive(Clone, Copy)]
pub(super) struct Rules<'a> {
    /// The kind the metadata belongs to.
    pub(super) kind: &'a str,
    /// The stages the kind admits.
    pub(super) lifecycle: &'a [Maturity],
    /// The known architecture 08 rows.
    pub(super) rows: &'a dyn KnownRows,
}

/// The metadata table a [`MetadataPlace::Strings`] map holds: its prefixed
/// keys without the prefix, its lists split. Unprefixed keys belong to the
/// format's ecosystem and are never read; one named like a Maestro key is
/// refused as ambiguous.
pub(super) fn from_strings(
    map: &Table,
    place: &MetadataPlace,
    key: &str,
    problems: &mut Problems,
) -> Table {
    let MetadataPlace::Strings {
        prefix, separator, ..
    } = place
    else {
        return map.clone();
    };
    let mut table = Table::new();
    for (name, value) in map {
        let Some(short) = name.strip_prefix(prefix.as_str()) else {
            if AMBIGUOUS.contains(&name.as_str()) {
                problems.push((
                    format!("{key}.{name}"),
                    format!("ambiguous; Maestro reads only {prefix}{name}"),
                ));
            }
            continue;
        };
        let Some(text) = value.text() else {
            problems.push((format!("{key}.{name}"), "must be a string".to_owned()));
            continue;
        };
        let value = if LISTS.contains(&short) {
            Value::List(
                text.split(*separator)
                    .map(|item| Value::Text(item.trim().to_owned()))
                    .collect(),
            )
        } else {
            Value::Text(text.to_owned())
        };
        table.insert(short.to_owned(), value);
    }
    table
}

/// The string `key` of `table`, noting it missing or mistyped.
fn text<'a>(table: &'a Table, key: &str, prefix: &str, problems: &mut Problems) -> Option<&'a str> {
    let value = table.get(key);
    let text = value.and_then(Value::text);
    if value.is_none() {
        problems.push((format!("{prefix}{key}"), "missing".to_owned()));
    } else if text.is_none() {
        problems.push((format!("{prefix}{key}"), "must be a string".to_owned()));
    }
    text
}

/// The list of strings `key` of `table`, empty when absent and not required.
fn list<'a>(table: &'a Table, key: &str, prefix: &str, problems: &mut Problems) -> Vec<&'a str> {
    let Some(value) = table.get(key) else {
        if key != "requires" {
            problems.push((format!("{prefix}{key}"), "missing".to_owned()));
        }
        return Vec::new();
    };
    value.texts().unwrap_or_else(|| {
        problems.push((
            format!("{prefix}{key}"),
            "must be a list of strings".to_owned(),
        ));
        Vec::new()
    })
}

/// The declared stage `name`, if the kind admits it.
fn stage(name: &str, rules: Rules<'_>) -> Result<Maturity, String> {
    let Some(maturity) = Maturity::ALL
        .into_iter()
        .find(|stage| stage.as_str() == name)
    else {
        return Err(format!("unknown maturity {name:?}"));
    };
    if rules.lifecycle.contains(&maturity) {
        Ok(maturity)
    } else if maturity == Maturity::Qualified {
        Err("qualified needs S4 evidence; a source cannot declare it".to_owned())
    } else {
        Err(format!("kind {} does not admit {name}", rules.kind))
    }
}

/// Reads and checks the metadata in `table`, its keys written under
/// `prefix`.
pub(super) fn metadata(
    table: &Table,
    prefix: &str,
    rules: Rules<'_>,
    problems: &mut Problems,
) -> Option<Metadata> {
    let key = |name: &str| format!("{prefix}{name}");
    for name in table.keys().filter(|name| !KEYS.contains(&name.as_str())) {
        problems.push((key(name), "unknown key".to_owned()));
    }
    let schema = text(table, "schema", prefix, problems);
    if let Some(schema) = schema.filter(|schema| *schema != SCHEMA) {
        problems.push((
            key("schema"),
            format!(
                "unsupported schema {schema:?}; this checker reads {SCHEMA}; \
                migrate the source envelope"
            ),
        ));
    }
    let owner = text(table, "owner", prefix, problems);
    if owner.is_some_and(|owner| owner.trim().is_empty()) {
        problems.push((key("owner"), "must name an owner".to_owned()));
    }
    let maturity = text(table, "maturity", prefix, problems).and_then(|name| {
        stage(name, rules)
            .map_err(|message| problems.push((key("maturity"), message)))
            .ok()
    });
    let rows = list(table, "rows", prefix, problems);
    row_problems(&rows, &key("rows"), rules.rows, problems);
    let workflows = list(table, "workflows", prefix, problems);
    workflow_problems(&workflows, &key("workflows"), problems);
    let requires = references(
        &list(table, "requires", prefix, problems),
        &key("requires"),
        problems,
    );
    let version = table.get("version").and_then(|value| {
        let version = value.text().filter(|version| !version.trim().is_empty());
        if version.is_none() {
            problems.push((key("version"), "must be a nonempty string".to_owned()));
        }
        version.map(str::to_owned)
    });
    Some(Metadata {
        owner: owner?.to_owned(),
        maturity: maturity?,
        rows: rows.into_iter().map(str::to_owned).collect(),
        workflows: workflows.into_iter().map(str::to_owned).collect(),
        requires,
        version,
    })
}

/// Checks the architecture 08 rows `rows` under `key`: at least one, each
/// known, none twice.
fn row_problems(rows: &[&str], key: &str, known: &dyn KnownRows, problems: &mut Problems) {
    if rows.is_empty() {
        problems.push((
            key.to_owned(),
            "must name at least one architecture 08 row".to_owned(),
        ));
    }
    for row in rows.iter().filter(|row| !known.knows(row)) {
        problems.push((
            key.to_owned(),
            format!("unknown architecture 08 row {row:?}"),
        ));
    }
    once(rows.iter().copied(), key, problems);
}

/// Checks the workflow names `workflows` under `key`: at least one, each a
/// name, none twice.
fn workflow_problems(workflows: &[&str], key: &str, problems: &mut Problems) {
    if workflows.is_empty() {
        problems.push((
            key.to_owned(),
            "must name at least one workflow it serves".to_owned(),
        ));
    }
    for workflow in workflows.iter().filter(|workflow| !is_name(workflow)) {
        problems.push((
            key.to_owned(),
            format!("{workflow:?} is not a lower-case hyphenated name"),
        ));
    }
    once(workflows.iter().copied(), key, problems);
}

/// The typed references `requires` writes under `key`, each checked, none
/// twice.
fn references(requires: &[&str], key: &str, problems: &mut Problems) -> Vec<ResourceId> {
    once(requires.iter().copied(), key, problems);
    requires
        .iter()
        .filter_map(|text| {
            let parsed = reference(text);
            if parsed.is_none() {
                problems.push((
                    key.to_owned(),
                    format!(
                        "{text:?} is not a qualified kind:namespace/local-name reference \
                        or area/preset root; migrate old IDs"
                    ),
                ));
            }
            parsed
        })
        .collect()
}

/// Qualified resources and the four root identity families; no legacy aliases.
fn reference(text: &str) -> Option<ResourceId> {
    let (kind, tail) = text.split_once(':')?;
    if !is_name(kind) || kind == "capability" {
        return None;
    }
    let root = matches!(kind, "package" | "language" | "standard" | "preset");
    let (namespace, name) = if root {
        (None, tail)
    } else {
        let (namespace, name) = tail.split_once('/')?;
        if !is_name(namespace) {
            return None;
        }
        (Some(namespace.to_owned()), name)
    };
    if !is_name(name) {
        return None;
    }
    Some(ResourceId {
        kind: kind.to_owned(),
        namespace,
        name: name.to_owned(),
    })
}
