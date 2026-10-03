//! Regression neighbours for area-scoped descriptors and shared package rules.

use super::{
    area_packages::{CARD, package_source},
    support::{MemoryTree, check_by, checked_model_card},
};
use crate::{limits::Limits, source::builtin};
use maestro_kernel::gateway::{
    CardIdentity, Role,
    card_v2::{Capability, Dimensions, EmbeddingFormat, Sampling, SamplingParameters},
};

#[test]
fn package_version_overlap_refuses_disagreement() {
    let registry = builtin().unwrap();
    for (kind, name, path) in [
        ("package", "core", "core/package.toml"),
        ("language", "rust", "languages/rust/package.toml"),
        ("standard", "security", "standards/security/package.toml"),
    ] {
        let base = package_source(kind, name);
        let results: Vec<_> = [None, Some("1.2.3"), Some("9.9.9"), Some("^9.9.9")]
            .into_iter()
            .map(|version| {
                let text = version.map_or_else(
                    || base.clone(),
                    |version| format!("{base}version = \"{version}\"\n"),
                );
                check_by(
                    &super::language::area_content(MemoryTree::default(), kind, name)
                        .with(path, &text),
                    &registry,
                    &Limits::PRODUCTION,
                )
            })
            .collect();
        assert_eq!(
            results.iter().map(Result::is_ok).collect::<Vec<_>>(),
            [true, true, false, false],
            "{kind}: {results:#?}"
        );
        for refusal in results.into_iter().skip(2) {
            assert!(
                refusal
                    .unwrap_err()
                    .to_string()
                    .contains("metadata.version")
            );
        }
    }
}

#[test]
fn package_name_diagnostic_names_the_area() {
    let refusal = check_by(
        &MemoryTree::default().with("core/package.toml", &package_source("package", "other")),
        &builtin().unwrap(),
        &Limits::PRODUCTION,
    )
    .unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("name: must equal the area name \"core\", not \"other\""),
        "{refusal}"
    );
}

#[test]
fn scoped_native_kinds_accept_registered_areas() {
    let registry = builtin().unwrap();
    let legacy = MemoryTree::valid();
    let team = "capabilities/practice/review";
    let agent = legacy.text("core/agents/valid.agent.md");
    let agent_metadata = legacy.text("core/agents/valid.maestro.toml").replace(
        "requires = [\"skill:common/valid-skill\", \"instructions:core/valid\"]",
        "requires = []",
    );
    let agent_tree = MemoryTree::default()
        .with(
            &format!("{team}/package.toml"),
            &package_source("package", "review"),
        )
        .with(&format!("{team}/agents/valid.agent.md"), &agent)
        .with(
            &format!("{team}/agents/valid.maestro.toml"),
            &agent_metadata,
        );
    let result = check_by(&agent_tree, &registry, &Limits::PRODUCTION);
    assert!(result.is_ok(), "team agent: {result:#?}");
    for area in ["core", team, "languages/rust", "standards/security"] {
        let tree = area_tree(area).with(
            &format!("{area}/skills/valid-skill/SKILL.md"),
            &legacy.text("skills/valid-skill/SKILL.md"),
        );
        let result = check_by(&tree, &registry, &Limits::PRODUCTION);
        assert!(result.is_ok(), "{area} skill: {result:#?}");
    }
    for area in [team, "languages/rust", "standards/security"] {
        let tree = area_tree(area)
            .with(
                &format!("{area}/instructions/valid.instructions.md"),
                &legacy.text("core/instructions/valid.instructions.md"),
            )
            .with(
                &format!("{area}/instructions/valid.maestro.toml"),
                &legacy.text("core/instructions/valid.maestro.toml"),
            );
        let result = check_by(&tree, &registry, &Limits::PRODUCTION);
        assert!(result.is_ok(), "{area} instructions: {result:#?}");
    }
    let tree = area_tree(team).with(&format!("{team}/llm/models/embedder/synthetic.toml"), CARD);
    let result = check_by(&tree, &registry, &Limits::PRODUCTION);
    assert!(result.is_ok(), "team card: {result:#?}");
}

#[test]
fn package_optional_maintainers_roundtrip() {
    let text = package_source("package", "core").replace(
        "description =",
        "maintainers = [\"@synthetic/maintainer\", \"@synthetic/other\"]\ndescription =",
    );
    let result = check_by(
        &MemoryTree::default().with("core/package.toml", &text),
        &builtin().unwrap(),
        &Limits::PRODUCTION,
    );
    assert!(result.is_ok(), "{result:#?}");
    assert_eq!(
        result.unwrap().resources[0].fields["maintainers"]
            .texts()
            .unwrap(),
        ["@synthetic/maintainer", "@synthetic/other"]
    );
}

/// An otherwise valid preset with area/inventory selectors.
fn preset_source() -> String {
    let package = package_source("package", "core");
    let metadata = package.split_once("[metadata]").unwrap().1;
    format!(
        "name = \"minimal\"\ndescription = \"Synthetic preset\"\n\
         templates = [\"common/base\", \"rust/starter\"]\n\
         [settings]\ntone = \"normal\"\n[metadata]{metadata}"
    )
}

#[test]
fn preset_missing_name_refuses() {
    let valid = preset_source();
    let registry = builtin().unwrap();
    let check = |text: &str| {
        check_by(
            &MemoryTree::owned().with("presets/minimal.toml", text),
            &registry,
            &Limits::PRODUCTION,
        )
    };
    assert!(check(&valid).is_ok());
    let result = check(&valid.replace("name = \"minimal\"\n", ""));
    assert!(result.is_err(), "missing name must refuse");
    assert!(result.unwrap_err().to_string().contains("name: missing"));
}

#[test]
fn preset_duplicate_templates_refuse() {
    let valid = preset_source();
    let registry = builtin().unwrap();
    let check = |text: &str| {
        check_by(
            &MemoryTree::owned().with("presets/minimal.toml", text),
            &registry,
            &Limits::PRODUCTION,
        )
    };
    assert!(check(&valid).is_ok());
    let result = check(&valid.replace("\"rust/starter\"", "\"common/base\""));
    assert!(result.is_err(), "duplicate templates must refuse");
    assert!(result.unwrap_err().to_string().contains("templates"));
}

#[test]
fn area_roots_require_reviewed_transitive_members() {
    let registry = builtin().unwrap();
    let skill = MemoryTree::valid().text("skills/valid-skill/SKILL.md");
    let middle = skill.replace("valid-skill", "middle-skill").replace(
        "  maestro.workflows:",
        "  maestro.requires: skill:common/leaf-skill\n  maestro.workflows:",
    );
    let leaf = skill.replace("valid-skill", "leaf-skill");
    for (kind, name, path) in [
        ("language", "rust", "languages/rust/package.toml"),
        ("standard", "security", "standards/security/package.toml"),
    ] {
        let root = package_source(kind, name).replace(
            "requires = []",
            "requires = [\"skill:common/middle-skill\"]",
        );
        let tree = super::language::area_content(MemoryTree::owned(), kind, name)
            .with(path, &root)
            .with("skills/middle-skill/SKILL.md", &middle)
            .with("skills/leaf-skill/SKILL.md", &leaf);
        assert!(check_by(&tree, &registry, &Limits::PRODUCTION).is_ok());
        let authored = tree.edit(
            "skills/leaf-skill/SKILL.md",
            "maestro.maturity: reviewed",
            "maestro.maturity: authored",
        );
        let result = check_by(&authored, &registry, &Limits::PRODUCTION);
        assert!(
            result.is_err(),
            "{kind} authored transitive member must refuse"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("closure member skill:common/leaf-skill is authored")
        );
    }
}

#[test]
fn role_card_matching_kernel_roles_accept() {
    let mut identity = checked_model_card(CARD).fields["identity"]
        .decode::<CardIdentity>()
        .unwrap();
    identity.invocation.dimensions = Dimensions::NotApplicable;
    identity.formats.embedding = EmbeddingFormat::NotApplicable;
    identity.formats.document = Capability::NotApplicable;
    identity.formats.query = Capability::NotApplicable;
    for role in [Role::Reranker, Role::Answerer] {
        identity.role = role;
        if role == Role::Answerer {
            identity.invocation.limits.output_tokens = Some(128.try_into().unwrap());
            identity.invocation.sampling = Sampling::Configured(SamplingParameters {
                temperature: 0.0,
                top_p: 1.0,
                top_k: 0,
                min_p: 0.0,
                typical_p: 1.0,
                repeat_penalty: 1.0,
                frequency_penalty: 0.0,
                presence_penalty: 0.0,
                seed: None,
            });
        }
        let mut declaration: toml::Table = toml::from_str(CARD).unwrap();
        declaration.insert(
            "identity".to_owned(),
            toml::Value::try_from(&identity).unwrap(),
        );
        let text = toml::to_string(&declaration).unwrap();
        let path = format!("core/llm/models/{role}/synthetic.toml");
        let result = check_by(
            &MemoryTree::owned().with(&path, &text),
            &builtin().unwrap(),
            &Limits::PRODUCTION,
        );
        assert!(result.is_ok(), "{role} matching identity: {result:#?}");
    }
}

/// The explicit descriptor at each native fixture's area boundary.
fn area_tree(area: &str) -> MemoryTree {
    let name = area.rsplit('/').next().unwrap();
    let kind = if area.starts_with("languages/") {
        "language"
    } else if area.starts_with("standards/") {
        "standard"
    } else {
        "package"
    };
    super::language::area_content(MemoryTree::default(), kind, name)
        .with(&format!("{area}/package.toml"), &package_source(kind, name))
}

#[test]
fn preset_template_selectors_refuse_paths_before_bootstrap() {
    let valid = preset_source();
    let registry = builtin().unwrap();
    for selector in [
        "common",
        "common/base/extra",
        "common/../base",
        "Common/base",
        "common/base.toml",
    ] {
        let result = check_by(
            &MemoryTree::owned().with(
                "presets/minimal.toml",
                &valid.replace("common/base", selector),
            ),
            &registry,
            &Limits::PRODUCTION,
        );
        assert!(result.is_err(), "accepted {selector}");
        assert!(result.unwrap_err().to_string().contains("area/inventory"));
    }
    assert!(
        check_by(
            &MemoryTree::owned().with("presets/minimal.toml", &valid),
            &registry,
            &Limits::PRODUCTION
        )
        .is_ok()
    );
}
