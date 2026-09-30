//! The kind registry: registrations stay distinct, each descriptor is
//! validated, a hook is selected by name from the fixed table, and every
//! built-in descriptor loaded back from data checks exactly as the original.

use super::support::{MemoryTree, assert_refused_by, check_by};
use crate::{
    limits::Limits,
    source::{
        Field, FieldType, Format, KindDescriptor, Layout, Maturity, MetadataPlace, Registry,
        builtin, builtin_hooks,
    },
};

/// A glossary kind, described as data only.
pub(super) fn glossary() -> KindDescriptor {
    KindDescriptor {
        kind: "glossary".to_owned(),
        version: 1,
        directory: "glossaries".to_owned(),
        layout: Layout::Files {
            suffix: ".toml".to_owned(),
            folders: vec![String::new()],
        },
        format: Format::Toml,
        metadata: MetadataPlace::Table {
            key: "metadata".to_owned(),
        },
        name_field: None,
        fields: vec![
            Field::required("term", FieldType::Text),
            Field::optional("synonyms", FieldType::TextList),
            Field::optional("weight", FieldType::Number),
            Field::optional(
                "source",
                FieldType::Table {
                    fields: vec![
                        Field::required("title", FieldType::Text),
                        Field::optional("page", FieldType::Integer),
                        Field::optional("public", FieldType::Boolean),
                    ],
                },
            ),
        ],
        body: false,
        requires: Vec::new(),
        lifecycle: vec![Maturity::Authored, Maturity::Reviewed],
        closure_root: false,
        required: None,
        hook: None,
    }
}

/// Registering `descriptor` beside the built-in kinds.
fn register(descriptor: KindDescriptor) -> Result<(), String> {
    builtin().unwrap().register(descriptor)
}

#[test]
fn registry_refuses_a_second_kind_or_directory() {
    let mut same_kind = glossary();
    same_kind.kind = "agent".to_owned();
    assert_eq!(
        register(same_kind),
        Err("kind \"agent\" is already registered".to_owned())
    );
    let mut same_directory = glossary();
    same_directory.directory = "skills".to_owned();
    assert_eq!(
        register(same_directory),
        Err("directory \"skills\" already has a kind".to_owned())
    );
}

/// A named change to the glossary descriptor and the refusal it earns.
type Case = (&'static str, fn(&mut KindDescriptor), &'static str);

#[test]
fn registry_refuses_a_descriptor_that_contradicts_itself() {
    let cases: Vec<Case> = vec![
        (
            "kind name",
            |descriptor| descriptor.kind = "Glossary".to_owned(),
            "kind Glossary: its name is not a lower-case hyphenated name",
        ),
        (
            "version zero",
            |descriptor| descriptor.version = 0,
            "kind glossary: its version must be 1 or more",
        ),
        (
            "directory never read",
            |descriptor| descriptor.directory = "docs".to_owned(),
            "kind glossary: directory \"docs\" is not one the catalog reads",
        ),
        (
            "directory not a name",
            |descriptor| descriptor.directory = ".glossaries".to_owned(),
            "kind glossary: directory \".glossaries\" is not one the catalog reads",
        ),
        (
            "sidecar with a single file",
            |descriptor| {
                descriptor.layout = Layout::Single {
                    file: "terms.toml".to_owned(),
                    name: "terms".to_owned(),
                };
                descriptor.metadata = MetadataPlace::Sidecar {
                    suffix: ".maestro.toml".to_owned(),
                };
            },
            "kind glossary: a sidecar pairs only with the files layout",
        ),
        (
            "body of a TOML kind",
            |descriptor| descriptor.body = true,
            "kind glossary: only a Markdown kind has a body",
        ),
        (
            "qualified stage",
            |descriptor| descriptor.lifecycle.push(Maturity::Qualified),
            "kind glossary: qualified needs S4 evidence; no S3 kind admits it",
        ),
        (
            "name field not a text field",
            |descriptor| descriptor.name_field = Some("synonyms".to_owned()),
            "kind glossary: name field \"synonyms\" is not one of its text fields",
        ),
        (
            "name field absent",
            |descriptor| descriptor.name_field = Some("title".to_owned()),
            "kind glossary: name field \"title\" is not one of its text fields",
        ),
        (
            "unknown hook",
            |descriptor| descriptor.hook = Some("custom".to_owned()),
            "kind glossary: unknown hook \"custom\"",
        ),
    ];
    for (name, change, expected) in cases {
        let mut descriptor = glossary();
        change(&mut descriptor);
        assert_eq!(register(descriptor), Err(expected.to_owned()), "{name}");
    }
}

#[test]
fn delegated_fields_require_the_named_registered_validator() {
    let mut descriptor = glossary();
    descriptor.fields.push(Field::required(
        "identity",
        FieldType::Delegated {
            validator: "model-card".to_owned(),
        },
    ));
    assert_eq!(
        register(descriptor),
        Err("kind glossary: delegated field \"identity\" requires hook \"model-card\"".to_owned())
    );
}

#[test]
fn registry_admits_a_name_field_that_is_a_text_field_and_a_known_hook() {
    let mut descriptor = glossary();
    descriptor.name_field = Some("term".to_owned());
    descriptor.hook = Some("preset-settings".to_owned());
    assert_eq!(register(descriptor), Ok(()));
}

/// The built-in kinds, each descriptor written as JSON, read back and
/// registered: hooks come from the names the data holds.
fn from_data() -> Registry {
    let mut registry = builtin_hooks();
    for registration in builtin().unwrap().registrations() {
        let json = serde_json::to_string(&registration.descriptor).unwrap();
        let loaded: KindDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, registration.descriptor);
        registry.register(loaded).unwrap();
    }
    registry
}

#[test]
fn builtin_kinds_loaded_from_data_check_like_the_originals() {
    let registry = from_data();
    let kinds: Vec<&str> = registry
        .registrations()
        .map(|registration| registration.descriptor.kind.as_str())
        .collect();
    assert_eq!(
        kinds,
        [
            "agent",
            "skill",
            "instructions",
            "mcp",
            "preset",
            "model-card",
            "settings"
        ]
    );
    assert_eq!(
        check_by(&MemoryTree::valid(), &registry, &Limits::PRODUCTION),
        check_by(
            &MemoryTree::valid(),
            &builtin().unwrap(),
            &Limits::PRODUCTION
        )
    );
    assert_refused_by(
        &registry,
        vec![
            (
                "agent hook",
                MemoryTree::valid().edit(
                    "agents/base/valid.agent.md",
                    "## Boundaries\n\nSynthetic data only; no other tool.\n",
                    "",
                ),
                "agents/base/valid.agent.md: body: expected the sections",
            ),
            (
                "preset hook",
                MemoryTree::valid().edit(
                    "presets/knowledge-client.toml",
                    "tone = \"normal\"",
                    "colour = \"blue\"",
                ),
                "presets/knowledge-client.toml: settings.colour: unknown setting",
            ),
            (
                "settings hook",
                MemoryTree::valid().edit(
                    "settings/classes.toml",
                    "additive = []",
                    "additive = []\nopen = []",
                ),
                "settings/classes.toml: classes.open: unknown class",
            ),
        ],
    );
}
