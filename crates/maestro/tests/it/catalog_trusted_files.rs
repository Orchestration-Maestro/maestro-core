//! Apply persists preferences only at the displayed, approved root.
use super::support::Home;
use maestro_kernel::artifact::Digest;
use std::{collections::BTreeMap, ffi::OsString, fs, path::Path, str, time::SystemTime};

/// Read every generated workspace/host deliverable. Only config language/tone may differ.
/// Internal ownership records are checked structurally for the exact config digest,
/// not excluded as another deliverable exception. No receipt/log fields are scrubbed.
fn deliverables(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, below: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(root.join(below)).unwrap() {
            let entry = entry.unwrap();
            let relative = below.join(entry.file_name());
            if relative == Path::new(".maestro-files") {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                visit(root, &relative, files);
            } else {
                files.insert(
                    relative.to_string_lossy().replace('\\', "/"),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, Path::new(""), &mut files);
    files
}

fn init(home: &Home, root: &Path, choices: &[String]) -> super::support::Ended {
    let catalog = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/catalog")
        .canonicalize()
        .unwrap();
    let mut args = vec![
        "--json",
        "init",
        "--catalog-dir",
        catalog.to_str().unwrap(),
        "--preset",
        "knowledge-client",
        "--apply",
    ];
    for choice in choices {
        args.extend(["--set", choice]);
    }
    home.run_in(root, &args)
}

#[test]
fn catalog_trusted_files_default_config_is_persisted_at_displayed_root() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let canonical = root.canonicalize().unwrap();
    let path = canonical.to_str().unwrap();
    assert_eq!(
        home.run(&["trust", "add", path, "--confirm-path", path])
            .code,
        Some(0)
    );
    let applied = init(&home, &root, &[]);
    assert_eq!(applied.code, Some(0), "{applied:?}");
    let document: serde_json::Value = serde_json::from_str(&applied.stdout).unwrap();
    assert_eq!(document["root"].as_str(), Some(path));
    assert_eq!(
        fs::read_to_string(root.join(".maestro/config.toml")).unwrap(),
        concat!(
            "schema = \"maestro-preferences/1\"\nlanguage = \"en\"\n",
            "tone = \"normal\"\nupdates = \"propose\"\n"
        )
    );
}

#[test]
fn catalog_trusted_files_bootstrap_without_config_is_not_already_applied() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let canonical = root.canonicalize().unwrap();
    let path = canonical.to_str().unwrap();
    assert_eq!(
        home.run(&["trust", "add", path, "--confirm-path", path])
            .code,
        Some(0)
    );
    assert_eq!(init(&home, &root, &[]).code, Some(0));
    // Model a bootstrap-only workspace: remove the separate config and its ownership.
    for entry in fs::read_dir(root.join(".maestro-files")).unwrap() {
        let entry = entry.unwrap();
        let record: toml::Value =
            toml::from_str(&fs::read_to_string(entry.path()).unwrap()).unwrap();
        if record["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"].as_str() == Some(".maestro/config.toml"))
        {
            fs::remove_file(entry.path()).unwrap();
        }
    }
    fs::remove_file(root.join(".maestro/config.toml")).unwrap();
    assert!(root.join(".maestro/project.toml").exists());
    let rerun = init(&home, &root, &[]);
    assert_eq!(rerun.code, Some(0), "{rerun:?}");
    let document: serde_json::Value = serde_json::from_str(&rerun.stdout).unwrap();
    assert_eq!(document["already_applied"], false);
    assert!(root.join(".maestro/config.toml").exists());
}

/// The workspace/host deliverable boundary permits only config language/tone changes;
/// kernel/internal ownership metadata must bind that exact config digest structurally.
/// There are no other deliverable exceptions and no receipt/log normalization here.
#[test]
fn catalog_trusted_files_matrix_nested_ancestor_and_identical_rerun() {
    let home = Home::bare();
    let ancestor = home.root().join("project");
    fs::create_dir_all(ancestor.join(".maestro")).unwrap();
    fs::write(
        ancestor.join(".maestro/config.toml"),
        "schema = 'maestro-preferences/1'\nlanguage = 'es'\n",
    )
    .unwrap();
    fs::write(ancestor.join(".maestro/user-note"), b"ancestor unchanged").unwrap();
    let ancestor_before = deliverables(&ancestor);
    let canonical = ancestor.canonicalize().unwrap();
    let path = canonical.to_str().unwrap();
    assert_eq!(
        home.run(&["trust", "add", path, "--confirm-path", path])
            .code,
        Some(0)
    );
    let mut baseline = None;
    for language in ["en", "fr", "es", "ja"] {
        for tone in ["brief", "normal", "detailed"] {
            let root = ancestor.join(format!("{language}-{tone}"));
            fs::create_dir(&root).unwrap();
            let normalized = apply_case(&home, &root, language, tone);
            if let Some(baseline) = &baseline {
                assert_eq!(&normalized, baseline);
            } else {
                baseline = Some(normalized);
            }
        }
    }
    for (name, bytes) in ancestor_before {
        assert_eq!(fs::read(ancestor.join(name)).unwrap(), bytes);
    }
}

/// Apply, verify exact ownership, replay without rewriting, and normalize only two config values.
fn apply_case(home: &Home, root: &Path, language: &str, tone: &str) -> BTreeMap<String, Vec<u8>> {
    let choices = [format!("language={language}"), format!("tone={tone}")];
    let result = init(home, root, &choices);
    assert_eq!(result.code, Some(0), "{result:?}");
    let before = deliverables(root);
    let config = before[".maestro/config.toml"].clone();
    let digest = format!("sha256:{}", Digest::of(&config).as_str());
    let records = ownership_records(root, &digest);
    let rerun = init(home, root, &choices);
    assert_eq!(rerun.code, Some(0), "{rerun:?}");
    let document: serde_json::Value = serde_json::from_str(&rerun.stdout).unwrap();
    assert!(document["already_applied"].as_bool().unwrap());
    assert_eq!(deliverables(root), before);
    for (name, bytes, modified) in records {
        let file = root.join(".maestro-files").join(name);
        assert_eq!(fs::read(&file).unwrap(), bytes);
        assert_eq!(fs::metadata(file).unwrap().modified().unwrap(), modified);
    }
    let mut normalized = before;
    let mut parsed: toml::Value = toml::from_str(str::from_utf8(&config).unwrap()).unwrap();
    assert_eq!(parsed["language"].as_str(), Some(language));
    assert_eq!(parsed["tone"].as_str(), Some(tone));
    parsed["language"] = toml::Value::String("en".into());
    parsed["tone"] = toml::Value::String("normal".into());
    normalized.insert(
        ".maestro/config.toml".into(),
        toml::to_string(&parsed).unwrap().into_bytes(),
    );
    normalized
}

/// Structural metadata assertion has no wildcard digest or language/tone exception.
fn ownership_records(root: &Path, digest: &str) -> Vec<(OsString, Vec<u8>, SystemTime)> {
    let mut count = 0;
    let mut records = Vec::new();
    for entry in fs::read_dir(root.join(".maestro-files")).unwrap() {
        let entry = entry.unwrap();
        let bytes = fs::read(entry.path()).unwrap();
        let record: toml::Value = toml::from_str(str::from_utf8(&bytes).unwrap()).unwrap();
        for file in record["files"].as_array().unwrap() {
            if file["path"].as_str() == Some(".maestro/config.toml") {
                assert_eq!(file["digest"].as_str(), Some(digest));
                count += 1;
            }
        }
        records.push((
            entry.file_name(),
            bytes,
            entry.metadata().unwrap().modified().unwrap(),
        ));
    }
    assert_eq!(count, 1);
    records
}
