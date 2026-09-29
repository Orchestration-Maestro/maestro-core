//! A kind described as data: where its files live, how they are written,
//! where its Maestro metadata sits, its typed fields and references, and the
//! lifecycle stages it admits, and the hook, if any, it selects by name.
//! Descriptors serialize, so a later step can load them from files without a
//! redesign; registering one validates it.

use super::types::Maturity;
use serde::{Deserialize, Serialize};

/// One kind of catalog resource, as data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindDescriptor {
    /// The kind's name, as `kind:name` references write it.
    pub kind: String,
    /// The version of the kind's source shape, from 1; a compiled catalog
    /// records it beside each resource.
    pub version: u32,
    /// The top-level catalog directory holding its resources.
    pub directory: String,
    /// How its files are laid out in that directory.
    pub layout: Layout,
    /// How its primary file is written.
    pub format: Format,
    /// Where its Maestro metadata sits.
    pub metadata: MetadataPlace,
    /// The field that must equal the resource's name, if any.
    pub name_field: Option<String>,
    /// The fields of its frontmatter or TOML document, beside the metadata.
    pub fields: Vec<Field>,
    /// Whether its Markdown body must hold text.
    pub body: bool,
    /// The kinds its `requires` may name; `*` admits every registered kind.
    pub requires: Vec<String>,
    /// The stages its sources may declare.
    pub lifecycle: Vec<Maturity>,
    /// Whether it roots a declared closure whose members must be reviewed.
    pub closure_root: bool,
    /// Why a catalog needs one, when it must hold at least one.
    pub required: Option<String>,
    /// The special-rule hook it selects from the fixed table, by name.
    pub hook: Option<String>,
}

/// How a kind's files are laid out in its directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub enum Layout {
    /// `<folder>/<name><suffix>` in each folder pattern, relative to the
    /// kind's directory: `""` is the directory itself, and a `*` segment
    /// matches any folder with a valid name.
    Files {
        /// The file name's suffix after the resource name.
        suffix: String,
        /// The folder patterns.
        folders: Vec<String>,
    },
    /// `<name>/<file>`: one folder per resource, whose `data` subfolders are
    /// kept as data, never read.
    Folder {
        /// The primary file's name.
        file: String,
        /// The subfolders kept as data.
        data: Vec<String>,
    },
    /// One file with a fixed name, the kind's one resource.
    Single {
        /// The file's name.
        file: String,
        /// The resource's name.
        name: String,
    },
}

/// How a primary file is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    /// Markdown with YAML frontmatter between `---` lines.
    Markdown,
    /// A TOML document; its `schema` sits with its metadata.
    Toml,
}

/// Where a kind's Maestro metadata sits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub enum MetadataPlace {
    /// A TOML sidecar `<name><suffix>` beside the primary file, for formats
    /// without an extension field.
    Sidecar {
        /// The sidecar's suffix after the resource name.
        suffix: String,
    },
    /// A table of the TOML document.
    Table {
        /// The table's key.
        key: String,
    },
    /// A string map in the frontmatter, the format's own extension field:
    /// Maestro's keys carry a prefix, and lists join items with a separator.
    Strings {
        /// The map's key.
        key: String,
        /// The prefix of Maestro's keys.
        prefix: String,
        /// The separator of list items.
        separator: char,
    },
}

/// One typed field of a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    /// Its key.
    pub key: String,
    /// Its type.
    pub kind: FieldType,
    /// Whether the document must hold it.
    pub required: bool,
}

impl Field {
    /// A field that must be present.
    #[must_use]
    pub fn required(key: &str, kind: FieldType) -> Self {
        Self {
            key: key.to_owned(),
            kind,
            required: true,
        }
    }

    /// A field that may be absent.
    #[must_use]
    pub fn optional(key: &str, kind: FieldType) -> Self {
        Self {
            key: key.to_owned(),
            kind,
            required: false,
        }
    }
}

/// The type of a field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub enum FieldType {
    /// A nonempty string.
    Text,
    /// An integer.
    Integer,
    /// A number: an integer or a finite fraction.
    Number,
    /// A boolean.
    Boolean,
    /// A list of nonempty strings, none twice.
    TextList,
    /// A list of nonempty strings in order, where a value may repeat.
    TextSequence,
    /// A list of tool names (ASCII lower-case letters, digits, `_` and `-`),
    /// none twice.
    ToolList,
    /// A table whose values are strings, numbers or booleans.
    ScalarTable,
    /// A table whose values are lists of strings.
    ListTable,
    /// A nested table with its own typed fields, kept whole for the kind's
    /// hook.
    Table {
        /// Its fields; every other key is unknown.
        fields: Vec<Field>,
    },
}
