//! The kind registry: registrations stay distinct, each descriptor is
//! validated, a hook is selected by name from the fixed table, and every
//! built-in descriptor loaded back from data checks exactly as the original.

use super::support::{MemoryTree, assert_refused_by, check_by};
use crate::source::builtin;
use crate::{
    limits::Limits,
    source::{
        Field, FieldType, Format, KindDescriptor, Layout, Maturity, MetadataPlace,
        RegistrationError, Registry, Scope, builtin_hooks,
    },
};

/// A glossary kind, described as data only.
pub(super) fn glossary() -> KindDescriptor {
    KindDescriptor {
        scopes: vec![Scope::Common],
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
    builtin()
        .unwrap()
        .register(descriptor)
        .map_err(|error| error.to_string())
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
        Err("kind glossary: overlapping placement".to_owned())
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
            |descriptor| descriptor.directory = "../docs".to_owned(),
            "kind glossary: unsafe relative placement",
        ),
        (
            "directory not a name",
            |descriptor| descriptor.directory = "glossaries/../other".to_owned(),
            "kind glossary: unsafe relative placement",
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
        Err(concat!(
            "kind glossary: delegated field \"identity\" requires a declared ",
            "top-level path for hook \"model-card\""
        )
        .to_owned())
    );
}

#[test]
fn registry_refuses_a_delegated_field_with_the_wrong_validator_name() {
    let mut matching = builtin()
        .unwrap()
        .kind("model-card")
        .unwrap()
        .descriptor
        .clone();
    matching.kind = "matching-card".to_owned();
    matching.directory = "matching-cards".to_owned();
    assert_eq!(register(matching.clone()), Ok(()));

    matching.fields[1].kind = FieldType::Delegated {
        validator: "preset-settings".to_owned(),
    };
    assert!(register(matching).unwrap_err().contains("identity"));
}

#[test]
fn registry_refuses_delegation_not_declared_by_selected_hook() {
    let model_card = builtin()
        .unwrap()
        .kind("model-card")
        .unwrap()
        .descriptor
        .clone();
    let mut cloned = model_card;
    cloned.kind = "smuggle".to_owned();
    cloned.directory = "smuggles".to_owned();
    cloned.fields.push(Field::required(
        "payload",
        FieldType::Delegated {
            validator: "model-card".to_owned(),
        },
    ));
    assert!(register(cloned).unwrap_err().contains("payload"));

    let mut glossary = glossary();
    glossary.hook = Some("settings-classes".to_owned());
    glossary.fields.push(Field::required(
        "blob",
        FieldType::Delegated {
            validator: "settings-classes".to_owned(),
        },
    ));
    assert!(register(glossary).unwrap_err().contains("blob"));
}

#[test]
fn registry_refuses_nested_delegated_field() {
    let mut descriptor = glossary();
    descriptor.hook = Some("model-card".to_owned());
    descriptor.fields.push(Field::required(
        "payload",
        FieldType::Table {
            fields: vec![Field::required(
                "identity",
                FieldType::Delegated {
                    validator: "model-card".to_owned(),
                },
            )],
        },
    ));
    assert!(
        register(descriptor)
            .unwrap_err()
            .contains("payload.identity")
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
            "package",
            "language",
            "standard",
            "standard-check",
            "standard-exception",
            "preset",
            "bootstrap-inventory",
            "model-card"
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
                    "core/agents/valid.agent.md",
                    "## Boundaries\n\nSynthetic data only; no other tool.\n",
                    "",
                ),
                "core/agents/valid.agent.md: body: expected the sections",
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
        ],
    );
}

#[test]
fn overlapping_or_escaping_placements_refuse() {
    use super::area_support::{folder, scoped};
    let mut registry = Registry::default();
    registry.register(scoped("glossaries", &["core"])).unwrap();
    let mut overlap = scoped("glossaries", &["core"]);
    overlap.kind = "other".to_owned();
    let result = registry.register(overlap);
    assert!(result.is_err(), "overlapping placements must refuse");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("overlapping placement")
    );
    for path in [
        "../glossaries",
        "glossaries/../other",
        "/glossaries",
        "glossaries//other",
        "glossaries\\other",
        "c:/glossaries",
    ] {
        assert!(
            Registry::default()
                .register(scoped(path, &["common"]))
                .is_err(),
            "{path}"
        );
    }
    for path in [
        "../outside.json",
        "/outside.json",
        "assets/../../outside.json",
        "assets/*",
    ] {
        assert!(
            Registry::default().register(folder(&[path])).is_err(),
            "{path}"
        );
    }
}

#[test]
fn overlapping_folder_patterns_refuse() {
    use super::area_support::scoped;
    let mut descriptor = scoped("glossaries", &["common"]);
    descriptor.layout = Layout::Files {
        suffix: ".toml".to_owned(),
        folders: vec!["*".to_owned(), "review".to_owned()],
    };
    let result = Registry::default().register(descriptor);
    assert!(result.is_err(), "overlapping folder patterns must refuse");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("overlapping placement")
    );
}

#[test]
fn root_support_and_scope_boundaries_refuse_unregistered_areas() {
    use super::area_support::scoped;
    for scope in ["common", "core", "team", "language", "standard"] {
        for segment in ["core", "capabilities", "languages", "standards"] {
            assert!(
                Registry::default()
                    .register(scoped(&format!("{segment}/glossaries"), &[scope]))
                    .is_err(),
                "{scope}/{segment}"
            );
        }
    }
    assert!(
        Registry::default()
            .register(scoped("unchecked", &["root"]))
            .is_err()
    );
}

#[test]
fn root_support_table_is_exact() {
    use crate::source::Scope;
    assert_eq!(
        Scope::SUPPORT_ROOTS,
        [
            "presets",
            "marketplace",
            "templates",
            "schemas",
            "fixtures",
            "docs",
            ".github"
        ]
    );
}

#[test]
fn nested_area_folder_patterns_and_product_kinds_refuse() {
    use super::area_support::scoped;
    let mut descriptor = scoped("glossaries", &["core"]);
    descriptor.layout = Layout::Files {
        suffix: ".toml".to_owned(),
        folders: vec!["languages/rust".to_owned()],
    };
    assert!(Registry::default().register(descriptor).is_err());
    let mut descriptor = scoped("glossaries", &["common"]);
    descriptor.kind = "ladybug".to_owned();
    assert!(Registry::default().register(descriptor).is_err());
}

#[test]
fn unsafe_layout_shapes_and_overlapping_inventory_refuse() {
    use super::area_support::{folder, scoped};
    for layout in [
        Layout::Files {
            suffix: String::new(),
            folders: vec![String::new()],
        },
        Layout::Files {
            suffix: "../.toml".to_owned(),
            folders: vec![String::new()],
        },
        Layout::Files {
            suffix: ".toml".to_owned(),
            folders: vec![],
        },
        Layout::Files {
            suffix: ".toml".to_owned(),
            folders: vec!["../outside".to_owned()],
        },
        Layout::Single {
            file: "../file.toml".to_owned(),
            name: "valid".to_owned(),
        },
    ] {
        let mut descriptor = scoped("glossaries", &["common"]);
        descriptor.layout = layout;
        assert!(Registry::default().register(descriptor).is_err());
    }
    assert!(
        Registry::default()
            .register(folder(&["assets/file.json", "assets/file.json"]))
            .is_err()
    );
    assert!(Registry::default().register(folder(&["SKILL.md"])).is_err());
}

#[test]
fn unscoped_descriptor_refuses_before_discovery() {
    for populated in [false, true] {
        let mut registry = Registry::default();
        if populated {
            registry.register(glossary()).unwrap();
        }
        let mut legacy = glossary();
        legacy.kind = "legacy-glossary".to_owned();
        legacy.directory = "terms".to_owned();
        legacy.scopes.clear();
        let result = registry.register(legacy);
        assert!(
            result.is_err(),
            "all unscoped descriptors must refuse before discovery"
        );
        let error = result.unwrap_err();
        assert_eq!(
            error,
            RegistrationError::LegacyDescriptor {
                kind: "legacy-glossary".to_owned()
            }
        );
        assert!(
            error.to_string().contains("migrate") && error.to_string().contains("legacy-glossary")
        );
        assert_eq!(registry.registrations().count(), usize::from(populated));
    }
}

#[test]
fn all_unscoped_custom_source_refuses() {
    let tree = MemoryTree::default().with(
        "glossaries/evidence.toml",
        &format!(
            "term = \"evidence\"\n[metadata]{}",
            super::area_packages::package_source("package", "common")
                .split_once("[metadata]")
                .unwrap()
                .1
        ),
    );
    let mut registry = Registry::default();
    let mut descriptor = glossary();
    descriptor.scopes.clear();
    let registration = registry.register(descriptor);
    assert!(
        registration.is_err(),
        "a custom /2 catalog cannot opt into legacy discovery"
    );
    let result = check_by(&tree, &registry, &Limits::PRODUCTION);
    assert!(result.is_err());
}
