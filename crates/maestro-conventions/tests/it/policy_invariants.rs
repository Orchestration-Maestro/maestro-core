//! The repository's policies, checked on every pull request by `cargo test`.
#![cfg(test)]
use maestro_conventions::{
    broken_links, counted_lines, names_a_personal_directory, repository_files, root,
};
use std::{fs, path::PathBuf};

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
