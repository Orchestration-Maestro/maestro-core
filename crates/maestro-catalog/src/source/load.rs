//! One discovered resource read from its files, as its kind's descriptor
//! says: bounded bytes, strict parsing, typed fields, metadata, name, body,
//! then the kind's own rules.

use super::{
    descriptor::{Format, Layout, MetadataPlace},
    metadata::{Rules, from_strings, metadata},
    ownership::{local, locate},
    parse::{Table, fields, split_frontmatter, toml_table, yaml_table},
    registry::Registration,
    tree::SourceTree,
    types::{Diagnostic, Known, Problems, Resource, ResourceId, Value},
    walk::Unit,
};
use crate::{limits::Limits, policy::schema::bound_json};

/// A resource read from its files, with where its metadata lives.
#[derive(Debug)]
pub(crate) struct Loaded {
    /// The resource.
    pub(crate) resource: Resource,
    /// The native primary input retained for the semantic compiler.
    pub(crate) native: Native,
    /// The file holding its metadata.
    pub(super) metadata_path: String,
    /// The key prefix of its metadata there.
    pub(super) prefix: String,
}

/// Native primary input. JSON is decoded once; Cedar is parsed by C19 once.
#[derive(Debug)]
pub(crate) enum Native {
    /// TOML/Markdown already represented in the resource's typed fields.
    None,
    /// Contract or Cedar schema JSON, retained without converting through TOML.
    Json(serde_json::Value),
    /// Policy source, never evaluated as an executable.
    Cedar(String),
}

/// What reading a resource needs besides its unit and kind.
#[derive(Clone, Copy)]
pub(super) struct Context<'a> {
    /// The catalog.
    pub(super) tree: &'a dyn SourceTree,
    /// The limits.
    pub(super) limits: &'a Limits,
    /// What the checker knows beyond the catalog.
    pub(super) known: Known<'a>,
}

/// The text of `path`, at most the source file limit.
fn read(context: Context<'_>, path: &str) -> Result<String, Diagnostic> {
    let limit = context.limits.source_file_bytes;
    let bytes = context
        .tree
        .read(path, limit)
        .map_err(|error| Diagnostic::unreadable(path, format!("cannot read: {error}")))?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > limit) {
        return Err(Diagnostic::new(
            path,
            "",
            format!("larger than {limit} bytes"),
        ));
    }
    String::from_utf8(bytes).map_err(|_| Diagnostic::new(path, "", "not UTF-8"))
}

/// The diagnostics of `problems` found in `path`.
fn located(path: &str, problems: Problems) -> impl Iterator<Item = Diagnostic> + '_ {
    problems
        .into_iter()
        .map(move |(key, message)| Diagnostic::new(path, key, message))
}

/// A primary file's top-level table and Markdown body.
fn document<'a>(
    text: &'a str,
    format: Format,
    limits: &Limits,
    problems: &mut Problems,
) -> (Option<Table>, Option<&'a str>, Native) {
    match format {
        Format::Toml => (
            toml_table(text, limits)
                .map_err(|problem| problems.push(problem))
                .ok(),
            None,
            Native::None,
        ),
        Format::Markdown => {
            let Some((frontmatter, body)) = split_frontmatter(text) else {
                problems.push((String::new(), "no frontmatter between --- lines".to_owned()));
                return (None, None, Native::None);
            };
            (
                yaml_table(frontmatter, limits)
                    .map_err(|problem| problems.push(problem))
                    .ok(),
                Some(body),
                Native::None,
            )
        }
        Format::Json => {
            let native = bound_json(text, limits)
                .and_then(|()| super::json::parse(text.as_bytes()))
                .map_or_else(
                    |message| {
                        problems.push((String::new(), message));
                        Native::None
                    },
                    Native::Json,
                );
            (Some(Table::new()), None, native)
        }
        Format::Cedar => (Some(Table::new()), None, Native::Cedar(text.to_owned())),
    }
}

/// Takes the metadata table out of `table`, where `place` keeps it inside
/// the primary file, with its key prefix. A sidecar place never reaches
/// here: registration pairs it with a layout that finds its sidecar.
fn embedded(
    table: Option<&mut Table>,
    place: &MetadataPlace,
    problems: &mut Problems,
) -> (Option<Table>, String) {
    let (MetadataPlace::Table { key } | MetadataPlace::Strings { key, .. }) = place else {
        return (None, String::new());
    };
    let prefix = match place {
        MetadataPlace::Strings { prefix, .. } => format!("{key}.{prefix}"),
        _ => format!("{key}."),
    };
    let taken = match table.map(|table| table.remove(key)) {
        Some(Some(Value::Table(map))) => Some(from_strings(&map, place, key, problems)),
        Some(Some(_)) => {
            problems.push((key.clone(), "must be a table".to_owned()));
            None
        }
        Some(None) => {
            problems.push((key.clone(), "missing".to_owned()));
            None
        }
        None => None,
    };
    (taken, prefix)
}

/// Reads the resource `unit` of the kind `registration`.
///
/// # Errors
///
/// Every diagnostic of its files.
pub(super) fn load(
    unit: &Unit,
    registration: &Registration,
    context: Context<'_>,
) -> Result<Loaded, Vec<Diagnostic>> {
    let descriptor = &registration.descriptor;
    let text = read(context, &unit.path).map_err(|diagnostic| vec![diagnostic])?;
    let sidecar = match &unit.sidecar {
        Some(path) => Some(read(context, path).map_err(|diagnostic| vec![diagnostic])?),
        None => None,
    };
    let mut primary = Problems::new();
    let mut beside = Problems::new();
    let (mut table, body, native) =
        document(&text, descriptor.format, context.limits, &mut primary);
    let (found, prefix) = match &sidecar {
        Some(sidecar) => (
            toml_table(sidecar, context.limits)
                .map_err(|problem| beside.push(problem))
                .ok(),
            String::new(),
        ),
        None => embedded(table.as_mut(), &descriptor.metadata, &mut primary),
    };
    if let Some(table) = &table {
        fields(table, &descriptor.fields, "", &mut primary);
    }
    if descriptor.body && body.is_some_and(|body| body.trim().is_empty()) {
        primary.push(("body".to_owned(), "is empty".to_owned()));
    }
    if let (Some(field), Some(table)) = (&descriptor.name_field, &table) {
        name_problem(field, table, unit, &descriptor.layout, &mut primary);
    }
    let rules = Rules {
        kind: &descriptor.kind,
        lifecycle: &descriptor.lifecycle,
        rows: context.known.rows,
    };
    let into = if sidecar.is_some() {
        &mut beside
    } else {
        &mut primary
    };
    let declared = found.and_then(|found| metadata(&found, &prefix, rules, into));
    let mut resource = table.zip(declared).map(|(fields, metadata)| Resource {
        id: ResourceId {
            kind: unit.kind.clone(),
            namespace: unit.namespace.clone(),
            name: unit.name.clone(),
        },
        path: unit.path.clone(),
        files: [Some(&unit.path), unit.sidecar.as_ref()]
            .into_iter()
            .flatten()
            .cloned()
            .collect(),
        data: unit.data.clone(),
        metadata,
        fields,
    });
    if let (Some(resource), Some(rules)) = (&mut resource, registration.rules) {
        rules.check_resource(resource, body, context.known, &mut primary);
        match rules.assets(resource) {
            Ok(assets) => resource.data.extend(assets),
            Err(message) => primary.push(("files".to_owned(), message)),
        }
    }
    if let Some(resource) = &resource {
        local(resource, descriptor, &mut primary);
    }
    locate(&unit.path, descriptor, &mut primary);
    locate(&unit.path, descriptor, &mut beside);
    let metadata_path = unit.sidecar.clone().unwrap_or_else(|| unit.path.clone());
    let mut diagnostics: Vec<Diagnostic> = located(&unit.path, primary).collect();
    diagnostics.extend(located(&metadata_path, beside));
    match resource {
        Some(resource) if diagnostics.is_empty() => Ok(Loaded {
            resource,
            native,
            metadata_path,
            prefix,
        }),
        // A failed read always says why; should a path ever fail silently,
        // the resource is still refused, never dropped unseen.
        _ => Err(if diagnostics.is_empty() {
            vec![Diagnostic::new(
                &unit.path,
                "",
                "cannot be read as its kind describes",
            )]
        } else {
            diagnostics
        }),
    }
}

/// Notes a name field that differs from the resource's name.
fn name_problem(
    field: &str,
    checked: &Table,
    unit: &Unit,
    layout: &Layout,
    problems: &mut Problems,
) {
    let Some(declared) = checked.get(field).and_then(Value::text) else {
        return;
    };
    if declared != unit.name {
        let what = match layout {
            Layout::Folder { .. } => "its directory",
            Layout::Area { .. } => "the area name",
            _ => "the file stem",
        };
        problems.push((
            field.to_owned(),
            format!("must equal {what} {:?}, not {declared:?}", unit.name),
        ));
    }
}
