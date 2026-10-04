//! Permanent discovery-only neighbours from the C30 review.
use super::{
    area_support::{discover, folder, scoped},
    support::MemoryTree,
};
use crate::{
    limits::Limits,
    source::{Directory, Layout, MetadataPlace, Registry, SourceTree},
};
use std::{fs, io};

fn path_case(path: &str, accepted: bool) {
    let scratch = maestro_test_scratch::scratch_directory().unwrap();
    fs::write(scratch.join("file.toml"), "inside").unwrap();
    fs::create_dir(scratch.join("inside")).unwrap();
    fs::write(scratch.join("inside/file.toml"), "inside").unwrap();
    let result = Directory::new(&scratch).read(path, 100);
    fs::remove_dir_all(scratch).unwrap();
    if accepted {
        assert_eq!(result.unwrap(), b"inside");
    } else {
        assert!(result.is_err_and(|error| error.kind() == io::ErrorKind::InvalidInput));
    }
}

fn wildcard_case(directory: &str, scopes: &[&str], path: &str, accepted: bool) {
    let mut descriptor = scoped(directory, scopes);
    descriptor.layout = Layout::Files {
        suffix: ".toml".to_owned(),
        folders: vec!["*".to_owned()],
    };
    let mut registry = Registry::default();
    let registration = registry.register(descriptor);
    let found = discover(
        &MemoryTree::default().with(path, "data"),
        &registry,
        &Limits::PRODUCTION,
    );
    let paths: Vec<_> = found.units.iter().map(|unit| unit.path.as_str()).collect();
    let diagnostics: Vec<_> = found.diagnostics.iter().map(ToString::to_string).collect();
    if accepted {
        assert!(registration.is_ok() && paths == [path] && diagnostics.is_empty());
    } else {
        assert!(registration.is_err() || !diagnostics.is_empty());
    }
}

fn host_case(product: &str, directory_case: bool) {
    let directory = if directory_case {
        format!("hosts/{product}")
    } else {
        "hosts".to_owned()
    };
    let file = if directory_case {
        "config.toml"
    } else {
        product
    };
    let path = format!("core/{directory}/{file}");
    let mut descriptor = scoped(&directory, &["core"]);
    descriptor.layout = Layout::Single {
        file: file.to_owned(),
        name: "valid".to_owned(),
    };
    let mut registry = Registry::default();
    let registration = registry.register(descriptor);
    let found = discover(
        &MemoryTree::default().with(&path, "data"),
        &registry,
        &Limits::PRODUCTION,
    );
    let paths: Vec<_> = found.units.iter().map(|unit| unit.path.as_str()).collect();
    let diagnostics: Vec<_> = found.diagnostics.iter().map(ToString::to_string).collect();
    if directory_case {
        assert!(registration.is_ok() && paths == [path.as_str()] && diagnostics.is_empty());
    } else {
        assert!(registration.is_err() || !diagnostics.is_empty());
    }
}

fn implicit_case(area: &str, accepted: bool) {
    let path = format!("skills/{area}/SKILL.md");
    let mut registry = Registry::default();
    let registration = registry.register(folder(&[]));
    let found = discover(
        &MemoryTree::default().with(&path, "data"),
        &registry,
        &Limits::PRODUCTION,
    );
    let paths: Vec<_> = found.units.iter().map(|unit| unit.path.as_str()).collect();
    let diagnostics: Vec<_> = found.diagnostics.iter().map(ToString::to_string).collect();
    if accepted {
        assert!(registration.is_ok() && paths == [path.as_str()] && diagnostics.is_empty());
    } else {
        assert!(registration.is_err() || !diagnostics.is_empty());
    }
}

#[test]
fn c30_review_probe_metadata_sidecar_nested() {
    let mut descriptor = scoped("glossaries", &["core"]);
    descriptor.metadata = MetadataPlace::Sidecar {
        suffix: "/languages/metadata.toml".to_owned(),
    };
    let mut registry = Registry::default();
    let registration = registry.register(descriptor);
    let tree = MemoryTree::default()
        .with("core/glossaries/valid.toml", "data")
        .with("core/glossaries/valid/languages/metadata.toml", "data");
    let found = discover(&tree, &registry, &Limits::PRODUCTION);
    let diagnostics: Vec<_> = found.diagnostics.iter().map(ToString::to_string).collect();
    assert!(registration.is_err() || !diagnostics.is_empty());
}

#[test]
fn c30_review_probe_path_relative() {
    path_case("file.toml", true);
}
#[test]
fn c30_review_probe_path_leading_slash() {
    path_case("/file.toml", false);
}
#[test]
fn c30_review_probe_path_double_leading_slash() {
    path_case("//file.toml", false);
}
#[test]
fn c30_review_probe_path_absolute_inside() {
    path_case("/inside/file.toml", false);
}
#[test]
fn c30_review_probe_path_trailing_slash() {
    path_case("file.toml/", false);
}
#[test]
fn c30_review_probe_path_dot_prefix() {
    path_case("./file.toml", false);
}
#[test]
fn c30_review_probe_path_double_separator() {
    path_case("inside//file.toml", false);
}
#[test]
fn c30_review_probe_wildcard_core() {
    wildcard_case(
        "glossaries",
        &["core"],
        "core/glossaries/core/valid.toml",
        false,
    );
}
#[test]
fn c30_review_probe_wildcard_capabilities() {
    wildcard_case(
        "glossaries",
        &["core"],
        "core/glossaries/capabilities/valid.toml",
        false,
    );
}
#[test]
fn c30_review_probe_wildcard_languages() {
    wildcard_case(
        "glossaries",
        &["core"],
        "core/glossaries/languages/valid.toml",
        false,
    );
}
#[test]
fn c30_review_probe_wildcard_standards() {
    wildcard_case(
        "glossaries",
        &["core"],
        "core/glossaries/standards/valid.toml",
        false,
    );
}
#[test]
fn c30_review_probe_wildcard_review() {
    wildcard_case(
        "glossaries",
        &["core"],
        "core/glossaries/review/valid.toml",
        true,
    );
}
#[test]
fn c30_review_probe_wildcard_common_core() {
    wildcard_case("", &["common"], "core/valid.toml", false);
}
#[test]
fn c30_review_probe_host_pi_file() {
    host_case("pi", false);
}
#[test]
fn c30_review_probe_host_pi_directory() {
    host_case("pi", true);
}
#[test]
fn c30_review_probe_host_claude_code_file() {
    host_case("claude-code", false);
}
#[test]
fn c30_review_probe_host_claude_code_directory() {
    host_case("claude-code", true);
}
#[test]
fn c30_review_probe_host_copilot_file() {
    host_case("copilot", false);
}
#[test]
fn c30_review_probe_host_copilot_directory() {
    host_case("copilot", true);
}
#[test]
fn c30_review_probe_implicit_core() {
    implicit_case("core", false);
}
#[test]
fn c30_review_probe_implicit_capabilities() {
    implicit_case("capabilities", false);
}
#[test]
fn c30_review_probe_implicit_languages() {
    implicit_case("languages", false);
}
#[test]
fn c30_review_probe_implicit_standards() {
    implicit_case("standards", false);
}
#[test]
fn c30_review_probe_implicit_review() {
    implicit_case("review", true);
}

#[test]
fn wildcard_legitimate_scope_prefixes_accept() {
    wildcard_case(
        "glossaries",
        &["team"],
        "capabilities/core/review/glossaries/review/valid.toml",
        true,
    );
    wildcard_case(
        "glossaries",
        &["language"],
        "languages/core/glossaries/review/valid.toml",
        true,
    );
    wildcard_case(
        "glossaries",
        &["standard"],
        "standards/core/glossaries/review/valid.toml",
        true,
    );
}

#[test]
fn wildcard_root_support_keeps_reserved_segments() {
    wildcard_case("docs", &["root"], "docs/core/valid.toml", true);
}

#[test]
fn root_support_scope_does_not_exempt_other_scopes() {
    wildcard_case(
        "docs",
        &["root", "core"],
        "core/docs/core/valid.toml",
        false,
    );
    wildcard_case("docs", &["root", "core"], "docs/core/valid.toml", true);
    wildcard_case(
        "docs",
        &["root", "core"],
        "core/docs/review/valid.toml",
        true,
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
