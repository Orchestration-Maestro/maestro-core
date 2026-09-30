//! The repository's policies, checked on every pull request by `cargo test`.
#![cfg(test)]

mod catalog_traceability;
mod s1_traceability;

use maestro_conventions::{
    broken_links, counted_lines, names_a_personal_directory, repository_files, root,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Every repository file that reads as UTF-8 text, with its contents.
fn text_files() -> Vec<(PathBuf, String)> {
    let root = root();
    repository_files(&root)
        .unwrap()
        .into_iter()
        .filter_map(|file| {
            let text = fs::read_to_string(root.join(&file)).ok()?;
            Some((file, text))
        })
        .collect()
}

#[test]
fn every_conventions_test_file_is_registered() {
    let tests = root().join("crates/maestro-conventions/tests");
    for file in repository_files(&tests).unwrap() {
        if file.extension().is_none_or(|extension| extension != "rs")
            || file == Path::new("policies.rs")
        {
            continue;
        }
        let module_path = if file.file_name().unwrap() == "mod.rs" {
            file.parent().unwrap().to_path_buf()
        } else {
            file.with_extension("")
        };
        let parent = module_path.parent().unwrap();
        // Root modules belong to the one test binary; nested modules to their parent.
        let declaring = if parent.as_os_str().is_empty() {
            PathBuf::from("policies.rs")
        } else {
            parent.join("mod.rs")
        };
        let source = fs::read_to_string(tests.join(&declaring)).unwrap();
        let module = module_path.file_name().unwrap().to_str().unwrap();
        assert!(
            source
                .lines()
                .any(|line| line.trim() == format!("mod {module};")),
            "{} must be declared as a module in {}",
            file.display(),
            declaring.display()
        );
    }
}

#[test]
fn rust_files_stay_within_500_counted_lines() {
    let oversized: Vec<String> = text_files()
        .into_iter()
        .filter(|(file, _)| file.extension().is_some_and(|extension| extension == "rs"))
        .filter_map(|(file, text)| {
            let lines = counted_lines(&text);
            (lines > 500).then(|| format!("{}: {lines}", file.display()))
        })
        .collect();
    assert!(oversized.is_empty(), "split these files: {oversized:?}");
}

#[test]
fn no_personal_path_in_repository_text() {
    let offenders: Vec<PathBuf> = text_files()
        .into_iter()
        .filter(|(_, text)| names_a_personal_directory(text))
        .map(|(file, _)| file)
        .collect();
    assert!(offenders.is_empty(), "personal paths in {offenders:?}");
}

#[test]
fn no_private_registry_or_quality_service_setting() {
    // Split so that this file does not name them. Documents may say what was
    // dropped; configuration and code may not carry it.
    let settings = [
        ["arti", "factory"].concat(),
        ["jf", "rog"].concat(),
        ["son", "ar"].concat(),
    ];
    let offenders: Vec<PathBuf> = text_files()
        .into_iter()
        .filter(|(file, _)| file.extension().is_none_or(|extension| extension != "md"))
        .filter(|(_, text)| {
            let text = text.to_lowercase();
            settings
                .iter()
                .any(|setting| text.contains(setting.as_str()))
        })
        .map(|(file, _)| file)
        .collect();
    assert!(
        offenders.is_empty(),
        "registry or quality-service settings in {offenders:?}"
    );
}

#[test]
fn every_member_inherits_the_workspace_lints() {
    for member in fs::read_dir(root().join("crates")).unwrap() {
        let manifest = member.unwrap().path().join("Cargo.toml");
        let text = fs::read_to_string(&manifest).unwrap();
        let name = manifest.display();
        assert!(
            text.contains("[lints]\nworkspace = true"),
            "{name} must inherit the lints"
        );
        assert!(
            !text.contains("[lints."),
            "{name} must not define its own lints"
        );
    }
}

#[test]
fn every_relative_link_and_anchor_resolves() {
    let root = root();
    let broken: Vec<String> = repository_files(&root)
        .unwrap()
        .into_iter()
        .filter(|file| file.extension().is_some_and(|extension| extension == "md"))
        // Markdown under examples/ is test input, kept byte for byte.
        .filter(|file| !file.components().any(|part| part.as_os_str() == "examples"))
        .flat_map(|file| broken_links(&root, &file).unwrap())
        .collect();
    assert!(broken.is_empty(), "broken links: {broken:?}");
}

/// Serialized opaque fields whose serde attributes omit the canonical adapter.
fn noncanonical_fields(source: &str) -> Vec<&str> {
    let mut attributes = String::new();
    let mut serialized = false;
    let mut offenders = Vec::new();
    for line in source.lines().map(str::trim) {
        if line.starts_with("#[derive(") {
            serialized = line.contains("Serialize");
        }
        if line.starts_with("///") || line.is_empty() {
            continue;
        }
        if line.starts_with("#[") || line.starts_with("serialize_with") {
            attributes.push_str(line);
            continue;
        }
        if serialized
            && line.contains(':')
            && line.ends_with(',')
            && line.contains("Value")
            && !attributes.contains("maestro_kernel::json::serialize_canonical")
        {
            offenders.push(line);
        }
        attributes.clear();
    }
    offenders
}

#[test]
fn opaque_canonical_fields_require_canonical_serialization() {
    for module in ["model", "document", "content", "dedup"] {
        let path = root().join(format!("crates/maestro-canonicalization/src/{module}.rs"));
        let source = fs::read_to_string(path).unwrap();
        assert!(
            noncanonical_fields(&source).is_empty(),
            "{module}: opaque fields lack canonical serialization: {:?}",
            noncanonical_fields(&source)
        );
    }
}

/// Unsafe operations and lint exceptions are confined to the held-handle Windows adapter.
#[test]
fn unsafe_is_confined_to_windows_handle_security() {
    let boundary = Path::new("crates/maestro-filesystem/src/windows_security.rs");
    let offenders: Vec<_> = text_files()
        .into_iter()
        .filter(|(file, _)| file.extension().is_some_and(|extension| extension == "rs"))
        .filter(|(file, _)| file != boundary)
        .filter(|(_, text)| unsafe_boundary_violation(text))
        .map(|(file, _)| file)
        .collect();
    assert!(
        offenders.is_empty(),
        "unsafe boundary violations: {offenders:?}"
    );
}

/// Ignore whitespace so formatting cannot evade the source-level boundary check.
fn unsafe_boundary_violation(text: &str) -> bool {
    let compact: String = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let unsafe_block = ["unsafe", "{"].concat();
    let lint = ["unsafe", "_code"].concat();
    compact.contains(&unsafe_block)
        || compact.split("allow(").skip(1).any(|attribute| {
            attribute
                .split(')')
                .next()
                .is_some_and(|lints| lints.contains(&lint))
        })
}

#[test]
fn unsafe_boundary_policy_rejects_formatted_blocks_and_lint_escape() {
    for planted in [
        ["unsafe", " { }"].concat(),
        ["unsafe", "\n{ }"].concat(),
        ["#[allow(", "unsafe", "_code)]"].concat(),
    ] {
        assert!(unsafe_boundary_violation(&planted));
    }
    assert!(!unsafe_boundary_violation("fn safe() {}"));
}
