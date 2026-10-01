//! The owner's scaling requirement: a new kind is one descriptor plus
//! fixtures. A glossary with a number and a nested table, and a model card
//! whose descriptor is read from JSON text, are checked end to end with no
//! checker change, and their nested tables reach a hook whole, readable
//! through serde.

use super::{
    registry::glossary,
    support::{MemoryTree, assert_refused_by, check_by},
};
use crate::source::builtin;
use crate::{
    limits::Limits,
    source::{Float, KindDescriptor, Registry, ResourceId, Value, builtin_hooks},
};
use serde::Deserialize;

/// The built-in kinds and `descriptor`.
fn with(descriptor: KindDescriptor) -> Registry {
    let mut registry = builtin_hooks();
    for registration in builtin().unwrap().registrations() {
        if registration.descriptor.kind != descriptor.kind {
            registry.register(registration.descriptor.clone()).unwrap();
        }
    }
    registry.register(descriptor).unwrap();
    registry
}

/// The metadata table of a reviewed synthetic resource.
const METADATA: &str = concat!(
    "[metadata]\n",
    "schema = \"maestro-source/2\"\n",
    "owner = \"@synthetic/knowledge\"\n",
    "maturity = \"reviewed\"\n",
    "rows = [\"chat.M036 objects\"]\n",
    "workflows = [\"ctm-question\"]\n",
);

/// The glossary entry's path.
const ENTRY: &str = "glossaries/evidence.toml";

/// The valid catalog with a glossary entry the preset requires.
fn glossary_catalog() -> MemoryTree {
    MemoryTree::valid()
        .with(
            ENTRY,
            &format!(
                "term = \"evidence\"\nsynonyms = [\"passage\"]\nweight = 0.6\n\n\
                    [source]\ntitle = \"Synthetic glossary\"\npage = 3\npublic = true\n\n\
                    {METADATA}"
            ),
        )
        .edit(
            "presets/knowledge-client.toml",
            "\"agent:core/valid\"]",
            "\"agent:core/valid\", \"glossary:common/evidence\"]",
        )
}

/// A glossary's source, as a hook would read it.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    /// Its title.
    title: String,
    /// Its page.
    page: u32,
    /// Whether it is public.
    public: bool,
}

#[test]
fn a_glossary_with_a_number_and_a_nested_table_is_one_descriptor() {
    let catalog = check_by(&glossary_catalog(), &with(glossary()), &Limits::PRODUCTION).unwrap();
    let entry = catalog
        .resources
        .iter()
        .find(|resource| resource.id.kind == "glossary")
        .unwrap();
    assert_eq!(
        entry.id,
        ResourceId {
            kind: "glossary".to_owned(),
            namespace: Some("common".to_owned()),
            name: "evidence".to_owned(),
        }
    );
    assert_eq!(entry.fields["term"], Value::Text("evidence".to_owned()));
    assert_eq!(
        entry.fields["weight"],
        Value::Float(Float::new(0.6).unwrap())
    );
    assert_eq!(
        entry.fields["source"].decode::<Source>(),
        Ok(Source {
            title: "Synthetic glossary".to_owned(),
            page: 3,
            public: true,
        })
    );
}

#[test]
fn glossary_neighbours_are_refused_by_the_generic_checks() {
    let tree = glossary_catalog();
    assert_refused_by(
        &builtin().unwrap(),
        vec![(
            "unregistered",
            tree.clone(),
            "glossaries/evidence.toml: not a registered v4 placement",
        )],
    );
    let edit = |from: &str, to: &str| tree.clone().edit(ENTRY, from, to);
    assert_refused_by(
        &with(glossary()),
        vec![
            (
                "unknown key",
                edit("term =", "colour = \"red\"\nterm ="),
                "glossaries/evidence.toml: colour: unknown key",
            ),
            (
                "missing field",
                edit("term = \"evidence\"\n", ""),
                "glossaries/evidence.toml: term: missing",
            ),
            (
                "weight not a number",
                edit("0.6", "\"heavy\""),
                "glossaries/evidence.toml: weight: must be a number",
            ),
            (
                "weight not finite",
                edit("0.6", "nan"),
                "glossaries/evidence.toml: weight: must be a finite number",
            ),
            (
                "weight infinite",
                edit("0.6", "-inf"),
                "glossaries/evidence.toml: weight: must be a finite number",
            ),
            (
                "page not an integer",
                edit("page = 3", "page = 3.5"),
                "glossaries/evidence.toml: source.page: must be an integer",
            ),
            (
                "public not a boolean",
                edit("public = true", "public = \"yes\""),
                "glossaries/evidence.toml: source.public: must be a boolean",
            ),
            (
                "nested unknown key",
                edit("page = 3", "page = 3\ncolour = \"red\""),
                "glossaries/evidence.toml: source.colour: unknown key",
            ),
            (
                "nested missing field",
                edit("title = \"Synthetic glossary\"\n", ""),
                "glossaries/evidence.toml: source.title: missing",
            ),
            (
                "nested table a string",
                edit("[source]\n", "source = \"book\"\n[other]\n"),
                "glossaries/evidence.toml: source: must be a table",
            ),
            (
                "date",
                edit("page = 3", "page = 2026-09-28"),
                "glossaries/evidence.toml: source.page: must be a string, number, boolean, list or \
                    table",
            ),
            (
                "stage the kind does not admit",
                edit("\"reviewed\"", "\"placeholder\""),
                "glossaries/evidence.toml: metadata.maturity: kind glossary does not admit \
                    placeholder",
            ),
            (
                "closure member",
                edit("\"reviewed\"", "\"authored\""),
                "presets/knowledge-client.toml: metadata.requires: closure member \
                    glossary:common/evidence is authored; a closure admits only reviewed members",
            ),
        ],
    );
}

/// A model card kind, as a descriptor file would hold it: a nested
/// identity, its sampling fractions and its version, no hook.
const MODEL_CARD: &str = r#"{
    "kind": "model-card",
    "version": 2,
    "directory": "llm/models",
    "scopes": ["core"],
    "layout": {"files": {"suffix": ".toml", "folders": ["*"]}},
    "format": "toml",
    "metadata": {"table": {"key": "metadata"}},
    "name_field": null,
    "fields": [
        {"key": "identity", "required": true, "kind": {"table": {"fields": [
            {"key": "role", "kind": "text", "required": true},
            {"key": "router_entry", "kind": "text", "required": true},
            {"key": "context_tokens", "kind": "integer", "required": true},
            {"key": "sampling", "required": true, "kind": {"table": {"fields": [
                {"key": "temperature", "kind": "number", "required": true},
                {"key": "top_p", "kind": "number", "required": true},
                {"key": "top_k", "kind": "integer", "required": true},
                {"key": "seed", "kind": "integer", "required": false}
            ]}}}
        ]}}}
    ],
    "body": false,
    "requires": [],
    "lifecycle": ["placeholder", "authored", "reviewed", "retired"],
    "closure_root": false,
    "required": null,
    "hook": null
}"#;

/// A model card's sampling, as the kernel's card types read it.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sampling {
    /// Sampling temperature.
    temperature: f64,
    /// Nucleus sampling probability limit.
    top_p: f64,
    /// Top-k sampling limit.
    top_k: u32,
    /// Explicit seed, when configured.
    seed: Option<u64>,
}

/// A model card's identity, as the kernel's card types read it.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    /// The card's role.
    role: String,
    /// The router's logical entry.
    router_entry: String,
    /// The context limit.
    context_tokens: u32,
    /// Its sampling.
    sampling: Sampling,
}

/// The card's path.
const CARD: &str = "core/llm/models/answerer/answerer.toml";

/// The valid catalog with a versioned model card the preset requires.
fn card_catalog() -> MemoryTree {
    MemoryTree::valid()
        .with(
            CARD,
            &format!(
                "[identity]\nrole = \"answerer\"\nrouter_entry = \"synthetic-answerer\"\n\
                    context_tokens = 8192\n\n[identity.sampling]\ntemperature = 0.7\n\
                    top_p = 0.95\ntop_k = 40\nseed = 42\n\n{METADATA}version = \"1.0.0\"\n"
            ),
        )
        .edit(
            "presets/knowledge-client.toml",
            "\"agent:core/valid\"]",
            "\"agent:core/valid\", \"model-card:core/answerer\"]",
        )
}

#[test]
fn a_model_card_kind_is_one_descriptor_read_from_text() {
    let descriptor: KindDescriptor = serde_json::from_str(MODEL_CARD).unwrap();
    let catalog = check_by(&card_catalog(), &with(descriptor), &Limits::PRODUCTION).unwrap();
    let card = catalog
        .resources
        .iter()
        .find(|resource| resource.id.kind == "model-card")
        .unwrap();
    assert_eq!(card.metadata.version.as_deref(), Some("1.0.0"));
    assert_eq!(card.files, [CARD]);
    assert_eq!(
        card.fields["identity"].decode::<Identity>(),
        Ok(Identity {
            role: "answerer".to_owned(),
            router_entry: "synthetic-answerer".to_owned(),
            context_tokens: 8192,
            sampling: Sampling {
                temperature: 0.7,
                top_p: 0.95,
                top_k: 40,
                seed: Some(42),
            },
        })
    );
}

#[test]
fn model_card_neighbours_are_refused_by_the_generic_checks() {
    let descriptor: KindDescriptor = serde_json::from_str(MODEL_CARD).unwrap();
    let edit = |from: &str, to: &str| card_catalog().edit(CARD, from, to);
    assert_refused_by(
        &with(descriptor),
        vec![
            (
                "fraction not finite",
                edit("0.7", "nan"),
                "core/llm/models/answerer/answerer.toml: identity.sampling.temperature: must \
                be a finite number",
            ),
            (
                "fraction a string",
                edit("0.95", "\"high\""),
                "core/llm/models/answerer/answerer.toml: identity.sampling.top_p: must be a number",
            ),
            (
                "missing sampling field",
                edit("top_k = 40\n", ""),
                "core/llm/models/answerer/answerer.toml: identity.sampling.top_k: missing",
            ),
            (
                "unknown identity key",
                edit("role =", "colour = \"red\"\nrole ="),
                "core/llm/models/answerer/answerer.toml: identity.colour: unknown key",
            ),
            (
                "empty version",
                edit("version = \"1.0.0\"", "version = \"\""),
                "core/llm/models/answerer/answerer.toml: metadata.version: must be a nonempty \
                string",
            ),
        ],
    );
}

#[test]
fn a_value_that_does_not_fit_the_hook_type_says_why() {
    let value = Value::Table(
        [("title".to_owned(), Value::Integer(3))]
            .into_iter()
            .collect(),
    );
    assert_eq!(
        value.decode::<Source>(),
        Err("invalid type: integer `3`, expected a string".to_owned())
    );
    assert_eq!(Float::new(f64::NAN), None);
    assert_eq!(Float::new(f64::INFINITY), None);
    assert_eq!(Float::new(-0.0), Float::new(0.0));
    assert_eq!(Float::new(-2.5).map(Float::get), Some(-2.5));
}

#[test]
fn scoped_tool_fields_keep_native_names_and_ordered_repeated_arguments() {
    use crate::source::{Field, FieldType};
    let mut descriptor = glossary();
    descriptor.fields.extend([
        Field::required("tools", FieldType::ToolList),
        Field::required("args", FieldType::TextSequence),
    ]);
    let registry = with(descriptor);
    let text = format!(
        "term = \"evidence\"\ntools = [\"knowledge_get\"]\nargs = [\"-v\", \"serve\", \
        \"-v\"]\n{METADATA}"
    );
    let tree = MemoryTree::default().with(ENTRY, &text);
    let catalog = check_by(&tree, &registry, &Limits::PRODUCTION).unwrap();
    assert_eq!(
        catalog.resources[0].fields["args"].texts(),
        Some(vec!["-v", "serve", "-v"])
    );
    assert_refused_by(
        &registry,
        vec![(
            "malformed tool name",
            tree.edit(ENTRY, "knowledge_get", "Bad Tool"),
            "glossaries/evidence.toml: tools: \"Bad Tool\" is not a tool name",
        )],
    );
}
