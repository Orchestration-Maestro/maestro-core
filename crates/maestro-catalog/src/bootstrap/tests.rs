use super::compose as compose_module;
use super::{Preset, PresetPort, inspect};
use crate::{
    files::{FileInput, apply_with_failure, recover, remove},
    limits::Limits,
};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process,
};

struct Presets;

impl PresetPort for Presets {
    fn resolve(&self, names: &[String]) -> Result<Vec<Preset>, String> {
        let files = BTreeMap::from([
            (
                ".github/copilot-instructions.md".to_owned(),
                b"knowledge\n".to_vec(),
            ),
            (
                ".maestro/recipes.json".to_owned(),
                b"{\"recipes\":[]}\n".to_vec(),
            ),
        ]);
        names
            .iter()
            .map(|name| match name.as_str() {
                "knowledge-client" => Ok(Preset {
                    name: name.clone(),
                    files: BTreeMap::from([(
                        ".github/copilot-instructions.md".to_owned(),
                        files[".github/copilot-instructions.md"].clone(),
                    )]),
                    source_files: BTreeMap::new(),
                    tools: Vec::new(),
                }),
                "rust-service" => Ok(Preset {
                    name: name.clone(),
                    files: BTreeMap::from([(
                        ".maestro/recipes.json".to_owned(),
                        files[".maestro/recipes.json"].clone(),
                    )]),
                    source_files: BTreeMap::new(),
                    tools: Vec::new(),
                }),
                _ => Err(format!("unknown preset {name}")),
            })
            .collect()
    }
}

#[test]
fn composes_base_and_rust_and_rejects_invalid_generated_json() {
    let composed = compose(
        &Presets,
        &["knowledge-client".into(), "rust-service".into()],
    );
    assert!(composed.is_ok(), "{composed:?}");
    let files = composed.unwrap_or_default();
    assert!(
        files
            .iter()
            .any(|file| file.path == ".github/copilot-instructions.md")
    );
    assert!(
        files
            .iter()
            .any(|file| file.path == ".maestro/recipes.json")
    );
}

#[test]
fn inspection_does_not_execute_repository_scripts() {
    let root = env::temp_dir().join(format!("maestro-bootstrap-{}", process::id()));
    drop(fs::remove_dir_all(&root));
    fs::create_dir_all(&root).expect("create repository");
    let marker = root.join("script-ran");
    fs::write(root.join("build.rs"), format!("touch {}", marker.display())).expect("script");
    let result = inspect(Path::new(&root));
    assert!(result.is_ok(), "{result:?}");
    assert!(!marker.exists());
    fs::remove_dir_all(root).expect("remove repository");
}

#[test]
fn generated_recipe_json_is_strict_and_preset_collision_refuses() {
    assert!(compose_module::validate_json("recipes.json", br#"{"x":1,"x":2}"#).is_err());
    let composed = compose(
        &Presets,
        &["knowledge-client".into(), "knowledge-client".into()],
    );
    assert!(composed.is_err());
}

#[test]
fn core_fixture_presets_compose_from_the_replaceable_directory_adapter() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/catalog")
        .canonicalize()
        .unwrap();
    let provider = super::DirectoryPresets::new(&root);
    let composed = compose(
        &provider,
        &["knowledge-client".into(), "rust-service".into()],
    );
    assert!(composed.is_ok(), "{composed:?}");
    let files = composed.unwrap_or_default();
    assert_eq!(files.len(), 2);
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = env::temp_dir().join(format!(
            "maestro-bootstrap-fix-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn catalog(&self, overlay: &str) -> PathBuf {
        let catalog = self.0.join("catalog");
        fs::create_dir_all(catalog.join(format!("bootstrap/{overlay}"))).unwrap();
        fs::write(
            catalog.join("bootstrap/custom.toml"),
            format!(
                "name = \"custom\"\noverlay = \"{overlay}\"\n\
             tools = []\nfiles = [\"{overlay}/target.md\"]\n"
            ),
        )
        .unwrap();
        fs::write(
            catalog.join(format!("bootstrap/{overlay}/target.md")),
            b"inert",
        )
        .unwrap();
        catalog
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

#[cfg(unix)]
#[test]
fn symlinked_template_file_and_directory_are_refused_without_writes() {
    use std::os::unix::fs::symlink;
    for directory in [false, true] {
        let scratch = Scratch::new();
        let catalog = scratch.catalog("base");
        let overlay = catalog.join("bootstrap/base");
        if directory {
            fs::rename(&overlay, catalog.join("secret")).unwrap();
            symlink(catalog.join("secret"), &overlay).unwrap();
        } else {
            fs::remove_file(overlay.join("target.md")).unwrap();
            fs::write(catalog.join("secret"), b"SECRET").unwrap();
            symlink(catalog.join("secret"), overlay.join("target.md")).unwrap();
        }
        let result = super::preview(
            &scratch.0,
            &super::DirectoryPresets::new(&catalog),
            &["custom".into()],
        );
        assert!(result.is_err(), "followed symlink: {result:?}");
        assert!(!scratch.0.join("target.md").exists());
        assert!(!scratch.0.join(".maestro").exists());
        assert!(!scratch.0.join(".maestro-files").exists());
    }
}

#[cfg(unix)]
#[test]
fn symlinked_manifest_is_refused_without_writes() {
    use std::os::unix::fs::symlink;
    let scratch = Scratch::new();
    let catalog = scratch.catalog("base");
    fs::rename(
        catalog.join("bootstrap/custom.toml"),
        catalog.join("secret.toml"),
    )
    .unwrap();
    symlink(
        catalog.join("secret.toml"),
        catalog.join("bootstrap/custom.toml"),
    )
    .unwrap();
    assert!(
        super::preview(
            &scratch.0,
            &super::DirectoryPresets::new(&catalog),
            &["custom".into()]
        )
        .is_err()
    );
    assert!(!scratch.0.join(".maestro").exists());
}

#[test]
fn template_size_at_limit_is_complete_and_one_past_is_refused() {
    let scratch = Scratch::new();
    let catalog = scratch.catalog("base");
    let limit = usize::try_from(Limits::PRODUCTION.source_file_bytes).unwrap();
    let bytes = vec![b'x'; limit];
    let template = catalog.join("bootstrap/base/target.md");
    fs::write(&template, &bytes).unwrap();
    let provider = super::DirectoryPresets::new(&catalog);
    let selected = ["custom".into()];
    let presets = provider.resolve(&selected).unwrap();
    assert_eq!(presets[0].files["target.md"], bytes);
    fs::write(&template, vec![b'x'; limit + 1]).unwrap();
    let error = super::preview(&scratch.0, &provider, &selected).unwrap_err();
    assert!(error.contains("larger than"), "{error}");
    assert!(!scratch.0.join(".maestro").exists());
}

#[test]
fn manifest_size_at_limit_parses_and_one_past_is_refused() {
    let scratch = Scratch::new();
    let catalog = scratch.catalog("base");
    let path = catalog.join("bootstrap/custom.toml");
    let mut bytes = fs::read(&path).unwrap();
    bytes.resize(
        usize::try_from(Limits::PRODUCTION.source_file_bytes).unwrap(),
        b' ',
    );
    fs::write(&path, &bytes).unwrap();
    let provider = super::DirectoryPresets::new(&catalog);
    assert!(provider.resolve(&["custom".into()]).is_ok());
    bytes.push(b' ');
    fs::write(&path, bytes).unwrap();
    let error = provider.resolve(&["custom".into()]).unwrap_err();
    assert!(error.contains("larger than"), "{error}");
}

#[test]
fn new_overlay_is_manifest_data_not_a_code_change() {
    let scratch = Scratch::new();
    let catalog = scratch.catalog("python");
    fs::write(
        catalog.join("bootstrap/custom.toml"),
        "name = \"custom\"\noverlay = \"python\"\ntools = []\nfiles = [\"python/target.md\"]\n",
    )
    .unwrap();
    let preview = super::preview(
        &scratch.0,
        &super::DirectoryPresets::new(&catalog),
        &["custom".into()],
    )
    .unwrap();
    super::apply(&scratch.0, &preview).unwrap();
    assert_eq!(fs::read(scratch.0.join("target.md")).unwrap(), b"inert");
}

#[test]
fn different_presets_shipping_the_same_path_are_refused() {
    let scratch = Scratch::new();
    let catalog = scratch.catalog("base");
    let manifest = fs::read_to_string(catalog.join("bootstrap/custom.toml"))
        .unwrap()
        .replace("custom", "second");
    fs::write(catalog.join("bootstrap/second.toml"), manifest).unwrap();
    let error = super::preview(
        &scratch.0,
        &super::DirectoryPresets::new(&catalog),
        &["custom".into(), "second".into()],
    )
    .unwrap_err();
    assert!(
        error.contains("preset file collision: target.md"),
        "{error}"
    );
    assert!(!scratch.0.join(".maestro").exists());
}

#[test]
fn base_only_descriptor_has_exact_authoring_keys_and_lock_reference() {
    let scratch = Scratch::new();
    let catalog = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/catalog")
        .canonicalize()
        .unwrap();
    let preview = super::preview(
        &scratch.0,
        &super::DirectoryPresets::new(&catalog),
        &["knowledge-client".into()],
    )
    .unwrap();
    assert!(!scratch.0.join(".maestro").exists());
    super::apply(&scratch.0, &preview).unwrap();
    let descriptor: toml::Table =
        toml::from_str(&fs::read_to_string(scratch.0.join(".maestro/project.toml")).unwrap())
            .unwrap();
    assert_eq!(
        descriptor.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "capabilities",
            "context_files",
            "lock",
            "mode",
            "presets",
            "schema"
        ]
    );
    assert_eq!(descriptor["mode"].as_str(), Some("authoring"));
    assert_eq!(
        descriptor["lock"].as_str(),
        Some(".maestro/authoring.lock.json")
    );
    assert_eq!(
        descriptor["presets"].as_array().unwrap(),
        &[toml::Value::String("knowledge-client".into())]
    );
    assert_eq!(
        fs::read(scratch.0.join(".github/copilot-instructions.md")).unwrap(),
        fs::read(catalog.join("bootstrap/base/.github/copilot-instructions.md")).unwrap()
    );
    assert!(!scratch.0.join(".maestro/recipes.json").exists());
}

#[test]
fn authoring_lock_binds_every_generated_file_and_source() {
    use sha2::{Digest as _, Sha256};
    use std::fmt::Write as _;
    let scratch = Scratch::new();
    let catalog = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/catalog")
        .canonicalize()
        .unwrap();
    let preview = super::preview(
        &scratch.0,
        &super::DirectoryPresets::new(&catalog),
        &["knowledge-client".into(), "rust-service".into()],
    )
    .unwrap();
    super::apply(&scratch.0, &preview).unwrap();
    let lock: serde_json::Value =
        serde_json::from_slice(&fs::read(scratch.0.join(".maestro/authoring.lock.json")).unwrap())
            .unwrap();
    for (field, root, expected) in [
        (
            "files",
            &scratch.0,
            vec![
                ".github/copilot-instructions.md",
                ".maestro/project.toml",
                ".maestro/recipes.json",
            ],
        ),
        (
            "sources",
            &catalog,
            vec![
                "bootstrap/base/.github/copilot-instructions.md",
                "bootstrap/knowledge-client.toml",
                "bootstrap/rust-service.toml",
                "bootstrap/rust/.maestro/recipes.json",
            ],
        ),
    ] {
        let entries = lock[field].as_array().unwrap();
        let mut paths = Vec::new();
        for entry in entries {
            let path = entry["path"].as_str().unwrap();
            paths.push(path);
            let bytes = fs::read(root.join(path)).unwrap();
            assert_eq!(
                entry["sha256"].as_str().unwrap(),
                format!(
                    "sha256:{}",
                    Sha256::digest(bytes)
                        .iter()
                        .fold(String::new(), |mut hex, byte| {
                            write!(&mut hex, "{byte:02x}").unwrap();
                            hex
                        })
                ),
                "{field}/{path}"
            );
        }
        paths.sort_unstable();
        assert_eq!(paths, expected);
    }
}

fn compose(port: &dyn PresetPort, names: &[String]) -> Result<Vec<FileInput>, String> {
    compose_module::compose_resolved(port.resolve(names)?)
}

#[test]
fn bootstrap_interrupted_apply_recovers_and_replay_journal_is_ephemeral() {
    let scratch = Scratch::new();
    let catalog = scratch.catalog("base");
    let provider = super::DirectoryPresets::new(&catalog);
    let names = ["custom".into()];
    let first = super::preview(&scratch.0, &provider, &names).unwrap();
    super::apply(&scratch.0, &first).unwrap();
    let replay = super::preview(&scratch.0, &provider, &names).unwrap();
    assert!(replay.plan.is_applied());
    remove(&scratch.0, replay.plan.id()).unwrap();
    assert!(apply_with_failure(&scratch.0, &replay.plan, Some(1)).is_err());
    assert!(!scratch.0.join("target.md").exists());
    recover(&scratch.0, replay.plan.id()).unwrap();
    assert_eq!(fs::read(scratch.0.join("target.md")).unwrap(), b"inert");
    assert!(
        super::preview(&scratch.0, &provider, &names)
            .unwrap()
            .plan
            .is_applied()
    );
}

#[test]
fn path_tool_candidates_cover_unix_and_windows_extensions() {
    use super::project::tool_candidates;
    assert_eq!(tool_candidates("shell", ""), [PathBuf::from("shell")]);
    assert_eq!(
        tool_candidates("shell", ".EXE;.CMD"),
        [
            PathBuf::from("shell"),
            PathBuf::from("shell.EXE"),
            PathBuf::from("shell.CMD")
        ]
    );
    assert_eq!(
        tool_candidates("shell.exe", ".EXE"),
        [PathBuf::from("shell.exe")]
    );
}

#[test]
fn overlays_and_tools_are_single_top_level_names() {
    let scratch = Scratch::new();
    let catalog = scratch.catalog("base");
    let provider = super::DirectoryPresets::new(&catalog);
    for (overlay, tool) in [
        ("nested/overlay", "tool"),
        ("..", "tool"),
        ("base", "../tool"),
        ("base", "tool --install"),
    ] {
        let manifest = format!(
            "name = \"custom\"\noverlay = \"{overlay}\"\ntools = [\"{tool}\"]\nfiles = []\n"
        );
        fs::write(catalog.join("bootstrap/custom.toml"), manifest).unwrap();
        assert!(
            provider.resolve(&["custom".into()]).is_err(),
            "accepted {overlay}/{tool}"
        );
    }
    assert!(!scratch.0.join(".maestro").exists());
}
