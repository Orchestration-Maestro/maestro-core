//! C31 scoped builtin placement and kernel-role neighbours.

use super::{
    area_support::discover,
    support::{MemoryTree, check_by},
};
use crate::{
    limits::Limits,
    source::{KindDescriptor, Registry, Scope, builtin, builtin_hooks},
};

/// A real declaration with the exact kernel fixture identity.
pub(super) const CARD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/catalog/model-cards/valid.toml"
));

#[test]
fn area_package_placement_roundtrips() {
    let builtin = builtin().unwrap();
    let mut loaded = builtin_hooks();
    for registration in builtin.registrations() {
        let bytes = serde_json::to_vec(&registration.descriptor).unwrap();
        let descriptor: KindDescriptor = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(descriptor, registration.descriptor);
        assert!(!descriptor.scopes.is_empty());
        loaded.register(descriptor).unwrap();
    }
    assert!(loaded.kind("mcp").is_none());
    let package = &loaded
        .kind("package")
        .expect("package closure roots")
        .descriptor;
    assert!(package.closure_root);
    let paths = [
        "package.toml",
        "core/package.toml",
        "capabilities/practice/review/package.toml",
        "languages/rust/package.toml",
        "standards/security/package.toml",
    ];
    let tree = paths
        .iter()
        .fold(MemoryTree::default(), |tree, path| tree.with(path, "data"));
    let found = discover(&tree, &loaded, &Limits::PRODUCTION);
    assert!(found.diagnostics.is_empty(), "{:#?}", found.diagnostics);
    let mut names: Vec<_> = found.units.iter().map(|unit| unit.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["common", "core", "review", "rust", "security"]);
}

#[test]
fn role_card_path_matches_identity() {
    let registry = builtin().unwrap();
    let tree = MemoryTree::default().with("core/llm/models/embedder/synthetic.toml", CARD);
    assert!(check_by(&tree, &registry, &Limits::PRODUCTION).is_ok());
    for role in ["answerer", "unknown"] {
        let path = format!("core/llm/models/{role}/synthetic.toml");
        let result = check_by(
            &MemoryTree::default().with(&path, CARD),
            &registry,
            &Limits::PRODUCTION,
        );
        assert!(result.is_err(), "mismatched path role must refuse: {role}");
        assert!(result.unwrap_err().to_string().contains("path role"));
    }
}

#[test]
fn ambiguous_area_descriptor_refuses() {
    let registry = builtin().unwrap();
    let mut overlap = registry.kind("skill").unwrap().descriptor.clone();
    overlap.kind = "other-skill".to_owned();
    let mut registry = registry;
    assert!(registry.register(overlap).is_err());
    let mut legacy = super::registry::glossary();
    legacy.scopes = vec![Scope::Core];
    let mut scoped = Registry::default();
    scoped.register(legacy).unwrap();
    let mut legacy = super::registry::glossary();
    legacy.kind = "legacy-glossary".to_owned();
    legacy.directory = "legacy-glossaries".to_owned();
    legacy.scopes.clear();
    assert!(scoped.register(legacy).is_err());
}

#[test]
fn unsupported_nonempty_kinds_and_configs_refuse() {
    let registry = builtin().unwrap();
    for path in [
        "mcp/server.toml",
        "core/backends/mcp/config.toml",
        "languages/rust/profiles/quality/rust.toml",
        "core/workflows/flow/workflow.md",
        "instructions/root.instructions.md",
        "agents/root.agent.md",
    ] {
        let found = discover(
            &MemoryTree::default().with(path, "data"),
            &registry,
            &Limits::PRODUCTION,
        );
        assert!(
            !found.diagnostics.is_empty(),
            "unsupported nonempty content: {path}"
        );
        assert!(
            found
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("not a registered v4 placement"))
        );
    }
}

/// A checked area root using the /2 source envelope.
pub(super) fn package_source(kind: &str, name: &str) -> String {
    format!(
        "kind = \"{kind}\"\nname = \"{name}\"\nversion = \"1.2.3\"\n\
         owners = [\"@synthetic/knowledge\"]\ndescription = \"Synthetic area\"\n\
         status = \"active\"\n\n[metadata]\nschema = \"maestro-source/2\"\n\
         owner = \"@synthetic/knowledge\"\nmaturity = \"reviewed\"\n\
         rows = [\"chat.M036 objects\"]\nworkflows = [\"ctm-question\"]\nrequires = []\n"
    )
}

#[test]
fn package_fields_and_path_refuse_invalid_neighbours() {
    let registry = builtin().unwrap();
    let valid = package_source("package", "core");
    let path = "core/package.toml";
    let check = |text: &str| {
        check_by(
            &MemoryTree::default().with(path, text),
            &registry,
            &Limits::PRODUCTION,
        )
    };
    assert!(check(&valid).is_ok(), "{:#?}", check(&valid));
    for (from, to, message) in [
        ("1.2.3", "^1.2.3", "exact SemVer"),
        (
            "owners = [\"@synthetic/knowledge\"]",
            "owners = []",
            "nonempty",
        ),
        ("status = \"active\"", "status = \"reviewed\"", "status"),
        ("name = \"core\"", "name = \"other\"", "name"),
        ("kind = \"package\"", "kind = \"language\"", "kind"),
    ] {
        let result = check(&valid.replace(from, to));
        assert!(result.is_err(), "must refuse {message}");
        assert!(result.unwrap_err().to_string().contains(message));
    }
    for (kind, name, path) in [
        ("package", "common", "package.toml"),
        (
            "package",
            "review",
            "capabilities/practice/review/package.toml",
        ),
        ("language", "rust", "languages/rust/package.toml"),
        ("standard", "security", "standards/security/package.toml"),
    ] {
        assert!(
            check_by(
                &MemoryTree::default().with(path, &package_source(kind, name)),
                &registry,
                &Limits::PRODUCTION
            )
            .is_ok()
        );
    }
}

#[test]
fn preset_area_inventory_selectors_keep_native_settings() {
    let text = format!(
        "name = \"minimal\"\ndescription = \"Synthetic preset\"\n\
         templates = [\"common/base\", \"rust/starter\"]\n[settings]\ntone = \"normal\"\n{}",
        package_source("package", "core")
            .split_once("[metadata]")
            .unwrap()
            .1
    );
    // The envelope must be a table, not additional settings.
    let text = text.replace("schema =", "[metadata]\nschema =");
    let result = check_by(
        &MemoryTree::default().with("presets/minimal.toml", &text),
        &builtin().unwrap(),
        &Limits::PRODUCTION,
    );
    assert!(result.is_ok(), "{result:#?}");
    let preset = &result.unwrap().resources[0];
    assert_eq!(
        preset.fields["templates"].texts().unwrap(),
        ["common/base", "rust/starter"]
    );
}

#[test]
fn builtin_scoped_shapes_preserve_native_metadata_and_hooks() {
    let legacy = MemoryTree::valid();
    let tree = MemoryTree::default()
        .with(
            "core/agents/valid.agent.md",
            &legacy.text("core/agents/valid.agent.md"),
        )
        .with(
            "core/agents/valid.maestro.toml",
            &legacy.text("core/agents/valid.maestro.toml"),
        )
        .with(
            "skills/valid-skill/SKILL.md",
            &legacy.text("skills/valid-skill/SKILL.md"),
        )
        .with(
            "core/instructions/valid.instructions.md",
            &legacy.text("core/instructions/valid.instructions.md"),
        )
        .with(
            "core/instructions/valid.maestro.toml",
            &legacy.text("core/instructions/valid.maestro.toml"),
        );
    let registry = builtin().unwrap();
    let checked = check_by(&tree, &registry, &Limits::PRODUCTION).unwrap();
    assert_eq!(checked.resources.len(), 3);
    assert_eq!(checked.resources[0].metadata.owner, "@synthetic/knowledge");
    let bad = tree.edit(
        "core/agents/valid.agent.md",
        "## Boundaries",
        "## Different",
    );
    assert!(
        check_by(&bad, &registry, &Limits::PRODUCTION)
            .unwrap_err()
            .to_string()
            .contains("expected the sections")
    );
    for kind in ["agent", "skill", "instructions", "preset", "model-card"] {
        assert_eq!(registry.kind(kind).unwrap().descriptor.version, 3);
    }
}

#[test]
fn area_root_layout_refuses_ambiguous_or_unsafe_descriptors() {
    use crate::source::Layout;
    let package = builtin()
        .unwrap()
        .kind("package")
        .unwrap()
        .descriptor
        .clone();
    for (directory, scopes, file) in [
        ("nested", vec![Scope::Core], "package.toml"),
        ("", vec![Scope::Root], "package.toml"),
        ("areas", vec![], "package.toml"),
        ("", vec![Scope::Core], "../package.toml"),
        ("", vec![Scope::Core], "nested/package.toml"),
        ("", vec![Scope::Core], "package:toml"),
        ("", vec![Scope::Core], ""),
    ] {
        let mut descriptor = package.clone();
        descriptor.directory = directory.to_owned();
        descriptor.scopes = scopes;
        descriptor.layout = Layout::Area {
            file: file.to_owned(),
        };
        assert!(builtin_hooks().register(descriptor).is_err());
    }
}

#[test]
fn package_legacy_owner_must_belong_to_authoritative_owners() {
    let registry = builtin().unwrap();
    let check = |text: &str| {
        check_by(
            &MemoryTree::default().with("core/package.toml", text),
            &registry,
            &Limits::PRODUCTION,
        )
    };
    let valid = package_source("package", "core");
    let mismatch = valid.replace(
        "owner = \"@synthetic/knowledge\"",
        "owner = \"@synthetic/other\"",
    );
    let refusal = check(&mismatch);
    assert!(
        refusal.is_err(),
        "a legacy owner outside owners must refuse"
    );
    assert!(
        refusal
            .unwrap_err()
            .to_string()
            .contains("metadata.owner: must belong to owners")
    );
    let second_owner = valid.replace(
        "owners = [\"@synthetic/knowledge\"]",
        "owners = [\"@synthetic/first\", \"@synthetic/knowledge\"]",
    );
    assert!(
        check(&second_owner).is_ok(),
        "membership must not mean first owner"
    );
}
