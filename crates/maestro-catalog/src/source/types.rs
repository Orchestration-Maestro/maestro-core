//! The catalog's authoring schema, `maestro-source/2`, as typed data.
//!
//! # Kinds
//!
//! Each kind is registered as a [`KindDescriptor`](super::KindDescriptor):
//! its directory, file layout, format, where its metadata sits, its typed
//! fields, the kinds its `requires` may name and the stages it admits. A
//! rule no descriptor can state is a [`KindRules`](crate::source::rules::KindRules) hook
//! registered beside it. Adding a kind is one descriptor plus fixtures.
//! Builtins use registered area-relative placements: agents in core/team,
//! skills in all areas, instructions in every area except common, and kernel
//! model cards in core/team `llm/models/<role>/`. Native metadata and sidecars
//! remain unchanged. `package.toml` roots common/core/team package closures and
//! separate language/standard closures. Presets live at `presets/<name>.toml`.
//! Unsupported nonempty kinds/configs refuse, including legacy MCP resources;
//! MCP config registration belongs to its own task. Inert assets require exact
//! inventories, never recursive directory exemptions.
//!
//! Resource identities use the single `/2` qualified envelope. Ownership
//! comes from the area's `package.toml` through [`Catalog::ownership`];
//! resource-local ownership and delegation declarations are refused.
//!
//! # Metadata
//!
//! Every resource declares `schema` (`maestro-source/2`), `maturity`
//! (a stage its kind admits: `placeholder`,
//! `authored`, `reviewed` or `retired` for every built-in kind), `rows`
//! (nonempty, known architecture 08 rows), `workflows` (nonempty workflow
//! names), optional `requires` (typed `kind:namespace/local-name` references) and an
//! optional nonempty `version`. A skill writes them as strings with a
//! `maestro.` prefix, since the Agent Skills specification makes `metadata`
//! a string map; its lists join items with `;`, which no row key contains.
//! Its keys without the prefix belong to the Agent Skills ecosystem and are
//! never read, but one named like a Maestro key (or `stage`) is refused as
//! ambiguous. Unknown and duplicate keys, wrong types and other schema
//! versions are refused.
//!
//! # Decisions (supervisor, 2026-09-28, C03)
//!
//! 1. Instructions carry a sidecar like agents: the Copilot instructions
//!    format has no extension field (a host format's own field where it has
//!    one, a sidecar only where it has none).
//! 2. The architecture 08 rows a resource may name are the 85 included
//!    `source_row` keys of `specs/003-catalog/traceability.json`, kept as
//!    `data/known-rows.txt` in this crate and passed to the checker through
//!    [`KnownRows`]; a test fails when the two differ.
//! 3. Every resource names the workflows it serves; an empty list is a
//!    resource unused by any workflow. Resolving the names against workflow
//!    graphs arrives with the workflow kind (C21, C22a).
//! 4. A preset roots a declared closure: every resource its `requires` and
//!    its members' hook edges reach, itself included, must be `reviewed`
//!    with area-derived ownership. `qualified` needs S4 evidence, so no S3 kind
//!    admits it. References are typed `kind:namespace/local-name`.
//! 5. S1 descriptors declare each setting's sole override class; preset
//!    `[settings]` values are checked against those canonical descriptors.
//! 6. The owner's scaling requirement: kinds are registered descriptors
//!    that serialize as data, so a later step can load them from files. A
//!    descriptor selects a hook by name from a fixed table; hooks exist
//!    only for the agent's sections and server references, the settings
//!    classes and the preset's setting keys. A new kind whose fields are
//!    text, numbers, booleans, lists and nested tables is one descriptor
//!    plus fixtures. Model-card `identity` alone delegates its complete
//!    nested shape to the kernel hook, without loosening other table checks.
//!
//! # Rulings (supervisor, 2026-09-28, C03 round two)
//!
//! 1. YAML is built node by node under the depth limit and a budget of
//!    twice the frontmatter's bytes in nodes, so aliases cannot expand it.
//! 2. A dependency cycle is one diagnostic per strongly connected
//!    component, and a refusal prints at most 1,000 diagnostics, then a
//!    count of the rest.
//! 3. Only a reviewed root's closure is checked; its members must all be
//!    reviewed.
//! 4. An agent names its MCP servers by reference (`mcp-servers`), never
//!    with an embedded launch; the projection renders the reviewed launch.
//! 5. MCP `args` is an ordered list that may repeat a value.

use serde::{Deserialize, Serialize, Serializer, de::DeserializeOwned};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};
use toml::de::Error as TomlError;

/// The one authoring schema version this checker reads.
pub const SCHEMA: &str = "maestro-source/2";

/// A resource's identity: its kind's registered name and its own name,
/// unique within its kind and namespace, written `kind:namespace/local-name`.
/// Area roots and global presets instead use `kind:name`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceId {
    /// The kind's registered name.
    pub kind: String,
    /// The owning area namespace, absent for area roots and global presets.
    pub namespace: Option<String>,
    /// The name: lower-case ASCII letters and digits in hyphen-separated
    /// words, at most 64 characters.
    pub name: String,
}

impl fmt::Display for ResourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(namespace) = &self.namespace {
            write!(formatter, "{}:{namespace}/{}", self.kind, self.name)
        } else {
            write!(formatter, "{}:{}", self.kind, self.name)
        }
    }
}

/// An evidence stage (architecture 03 §1.2). A kind's descriptor lists the
/// stages its sources may declare; no S3 kind lists `qualified`, which needs
/// S4 evidence, never a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Maturity {
    /// Named, not yet written; discoverable, never in a closure.
    Placeholder,
    /// Written, not yet reviewed; discoverable, never in a closure.
    Authored,
    /// Declared reviewed with area ownership: may enter a closure.
    Reviewed,
    /// Qualified by S4 evidence.
    Qualified,
    /// Withdrawn; never in a closure.
    Retired,
}

impl Maturity {
    /// The stages an S3 source may declare: all but `qualified`.
    pub const DECLARABLE: [Self; 4] = [
        Self::Placeholder,
        Self::Authored,
        Self::Reviewed,
        Self::Retired,
    ];

    /// Every stage, in lifecycle order.
    pub const ALL: [Self; 5] = [
        Self::Placeholder,
        Self::Authored,
        Self::Reviewed,
        Self::Qualified,
        Self::Retired,
    ];

    /// The stage as sources write it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Placeholder => "placeholder",
            Self::Authored => "authored",
            Self::Reviewed => "reviewed",
            Self::Qualified => "qualified",
            Self::Retired => "retired",
        }
    }
}

/// The Maestro data every resource declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    /// The declared stage.
    pub maturity: Maturity,
    /// The architecture 08 rows the resource serves.
    pub rows: Vec<String>,
    /// The workflows the resource serves.
    pub workflows: Vec<String>,
    /// The resources it depends on.
    pub requires: Vec<ResourceId>,
    /// Its own version, when it declares one.
    pub version: Option<String>,
}

/// A finite floating-point number, kept as its bits so values stay `Eq`:
/// NaN and the infinities are never values, and `-0.0` is kept as `0.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Float(u64);

impl Float {
    /// `number`, if it is finite.
    #[must_use]
    pub fn new(number: f64) -> Option<Self> {
        number.is_finite().then(|| Self((number + 0.0).to_bits()))
    }

    /// The number.
    #[must_use]
    pub const fn get(self) -> f64 {
        f64::from_bits(self.0)
    }
}

/// A checked field value: TOML and YAML read into the few shapes sources use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A string.
    Text(String),
    /// An integer.
    Integer(i64),
    /// A finite fraction.
    Float(Float),
    /// A boolean.
    Boolean(bool),
    /// A list.
    List(Vec<Value>),
    /// A table with string keys.
    Table(BTreeMap<String, Value>),
}

impl Value {
    /// The string this value is, if it is one.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The strings this list holds, if it is a list of strings.
    #[must_use]
    pub fn texts(&self) -> Option<Vec<&str>> {
        match self {
            Self::List(items) => items.iter().map(Self::text).collect(),
            _ => None,
        }
    }

    /// This value read as `T` through serde, the way a hook reads a nested
    /// table into its own typed shape.
    ///
    /// # Errors
    ///
    /// Why the value does not fit `T`.
    pub fn decode<T: DeserializeOwned>(&self) -> Result<T, String> {
        toml::Value::try_from(self)
            .map_err(|error| error.to_string())?
            .try_into()
            .map_err(|error: TomlError| error.message().to_owned())
    }
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Text(text) => serializer.serialize_str(text),
            Self::Integer(integer) => serializer.serialize_i64(*integer),
            Self::Float(float) => serializer.serialize_f64(float.get()),
            Self::Boolean(flag) => serializer.serialize_bool(*flag),
            Self::List(items) => serializer.collect_seq(items),
            Self::Table(table) => serializer.collect_map(table),
        }
    }
}

/// Problems found in one file: each a dotted key, empty for the whole file,
/// and a message.
pub type Problems = Vec<(String, String)>;

/// One checked resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    /// Its identity.
    pub id: ResourceId,
    /// Its primary file, relative to the catalog, `/`-separated.
    pub path: String,
    /// Every file it owns, its primary file first, then its sidecar.
    pub files: Vec<String>,
    /// The folders it keeps as data, never read: a later step copies them.
    pub data: Vec<String>,
    /// Its Maestro data.
    pub metadata: Metadata,
    /// The fields its kind's descriptor declares, as the file writes them.
    pub fields: BTreeMap<String, Value>,
}

/// A checked catalog: its resources, sorted by ID.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    /// The resources.
    pub resources: Vec<Resource>,
}

/// What a diagnostic reports: a source the schema refuses, or a file the
/// checker could not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cause {
    /// The source breaks a rule.
    Refused,
    /// Reading or listing failed: the check itself failed.
    Unreadable,
}

/// One precise reason a catalog is refused.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Diagnostic {
    /// The file, relative to the catalog and `/`-separated; empty for the
    /// catalog as a whole.
    pub path: String,
    /// The key within the file, dotted; empty for the file as a whole.
    pub key: String,
    /// What is wrong.
    pub message: String,
    /// Whether the source is refused or could not be read.
    pub cause: Cause,
}

impl Diagnostic {
    /// A refusal on `key` of `path`.
    pub fn new(
        path: impl Into<String>,
        key: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            key: key.into(),
            message: message.into(),
            cause: Cause::Refused,
        }
    }

    /// `path` could not be read or listed, as `message` says.
    pub fn unreadable(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            cause: Cause::Unreadable,
            ..Self::new(path, "", message)
        }
    }
}

/// `text` with each control character written as its `\u{..}` escape, so a
/// diagnostic stays one line and cannot steer a terminal.
fn escaped(text: &str) -> String {
    text.chars().fold(String::new(), |mut out, character| {
        if character.is_control() {
            out.extend(character.escape_unicode());
        } else {
            out.push(character);
        }
        out
    })
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = if self.path.is_empty() {
            "catalog".to_owned()
        } else {
            escaped(&self.path)
        };
        let message = escaped(&self.message);
        if self.key.is_empty() {
            write!(formatter, "{path}: {message}")
        } else {
            write!(formatter, "{path}: {}: {message}", escaped(&self.key))
        }
    }
}

/// Why a catalog is refused: every diagnostic found, sorted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

impl Refusal {
    /// Whether a file or directory could not be read, so the check failed
    /// rather than refused the catalog.
    #[must_use]
    pub fn unreadable(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.cause == Cause::Unreadable)
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lines: Vec<String> = self.diagnostics.iter().map(ToString::to_string).collect();
        formatter.write_str(&lines.join("\n"))
    }
}

impl Error for Refusal {}

/// The port through which the checker learns which architecture 08 rows
/// exist; tests inject small sets.
pub trait KnownRows {
    /// Whether `row` is a known row key.
    fn knows(&self, row: &str) -> bool;
}

impl KnownRows for BTreeSet<String> {
    fn knows(&self, row: &str) -> bool {
        self.contains(row)
    }
}

/// The port through which the checker learns which settings Maestro knows;
/// S1's settings registry supplies them later, tests inject small sets.
pub trait KnownSettings {
    /// Every known setting key, sorted.
    fn keys(&self) -> Vec<&str>;
}

impl KnownSettings for BTreeSet<String> {
    fn keys(&self) -> Vec<&str> {
        self.iter().map(String::as_str).collect()
    }
}

impl KnownSettings for maestro_settings::Registry {
    fn keys(&self) -> Vec<&str> {
        let mut keys: Vec<_> = self
            .descriptors()
            .map(|descriptor| descriptor.key.as_ref())
            .collect();
        keys.sort_unstable();
        keys
    }
}

/// What the checker knows beyond the catalog: its ports.
#[derive(Clone, Copy)]
pub struct Known<'a> {
    /// The known architecture 08 rows.
    pub rows: &'a dyn KnownRows,
    /// The known settings.
    pub settings: &'a dyn KnownSettings,
}

impl fmt::Debug for Known<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Known")
            .field("settings", &self.settings.keys())
            .finish_non_exhaustive()
    }
}

/// The rows frozen by the S3 traceability inventory, one key per line.
const FROZEN_ROWS: &str = include_str!("../../data/known-rows.txt");

/// The architecture 08 rows the S3 inventory includes, the default
/// [`KnownRows`] adapter.
#[must_use]
pub fn frozen_rows() -> BTreeSet<String> {
    FROZEN_ROWS.lines().map(str::to_owned).collect()
}
