//! Project composition and C04 writer regressions on checked owner-local fixtures.
use super::compose as compose_module;
use super::{
    Preset, PresetPort, inspect,
    support::{apply, checked_port, checked_preview, copy_catalog, preview},
};
use crate::{
    files::{
        FileInput, digest,
        tests::support::{apply_with_failure, recover, remove},
    },
    limits::Limits,
};
use maestro_test_scratch::scratch_directory;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
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
                "base" => Ok(Preset {
                    name: name.clone(),
                    files: BTreeMap::from([(
                        ".github/copilot-instructions.md".to_owned(),
                        files[".github/copilot-instructions.md"].clone(),
                    )]),
                    source_files: BTreeMap::new(),
                    areas: Vec::new(),
                    tools: Vec::new(),
                    bindings: Vec::new(),
                    source_root: None,
                }),
                "rust" => Ok(Preset {
                    name: name.clone(),
                    files: BTreeMap::from([(
                        ".maestro/recipes.json".to_owned(),
                        files[".maestro/recipes.json"].clone(),
                    )]),
                    source_files: BTreeMap::new(),
                    areas: Vec::new(),
                    tools: Vec::new(),
                    bindings: Vec::new(),
                    source_root: None,
                }),
                _ => Err(format!("unknown preset {name}")),
            })
            .collect()
    }
}

#[test]
fn composes_base_and_rust_and_rejects_invalid_generated_json() {
    let composed = compose(&Presets, &["base".into(), "rust".into()]);
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
    let root = scratch_directory().unwrap();
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
    let composed = compose(&Presets, &["base".into(), "base".into()]);
    assert!(composed.is_err());
}

#[test]
fn core_fixture_presets_compose_from_the_replaceable_directory_adapter() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/catalog/bootstrap/owner-local")
        .canonicalize()
        .unwrap();
    let provider = checked_port(&root).unwrap();
    let composed = compose(&provider, &["base".into(), "rust".into()]);
    assert!(composed.is_ok(), "{composed:?}");
    let files = composed.unwrap_or_default();
    assert_eq!(files.len(), 2);
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        Self(scratch_directory().unwrap())
    }

    fn catalog(&self, inventory: &str) -> PathBuf {
        let catalog = self.0.join("catalog");
        copy_catalog(&catalog);
        let preset = fs::read_to_string(catalog.join("presets/base.toml"))
            .unwrap()
            .replace(r#"name = "base""#, r#"name = "custom""#)
            .replace("common/base", &format!("common/{inventory}"));
        fs::write(catalog.join("presets/custom.toml"), preset).unwrap();
        if inventory != "base" {
            fs::rename(
                catalog.join("bootstrap/base"),
                catalog.join(format!("bootstrap/{inventory}")),
            )
            .unwrap();
            fs::rename(
                catalog.join("bootstrap/base.toml"),
                catalog.join(format!("bootstrap/{inventory}.toml")),
            )
            .unwrap();
            // No stale preset may name an inventory that no longer exists.
            for name in ["base", "rust"] {
                let path = catalog.join(format!("presets/{name}.toml"));
                let text = fs::read_to_string(&path)
                    .unwrap()
                    .replace("common/base", &format!("common/{inventory}"));
                fs::write(path, text).unwrap();
            }
        }
        let files = catalog.join(format!("bootstrap/{inventory}/files"));
        fs::remove_file(files.join("instructions.md")).unwrap();
        fs::write(files.join("target.md"), b"inert").unwrap();
        let manifest = catalog.join(format!("bootstrap/{inventory}.toml"));
        let original = fs::read_to_string(&manifest).unwrap();
        let metadata = original.split_once("[metadata]").unwrap().1;
        let document = format!(
            concat!(
                "name = \"{}\"\nbindings = []\ntools = []\n[[files]]\n",
                "source = \"target.md\"\noutput = \"target.md\"\n",
                "sha256 = \"{}\"\n[metadata]{}"
            ),
            inventory,
            digest(b"inert"),
            metadata
        );
        fs::write(manifest, document).unwrap();
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
        let files = catalog.join("bootstrap/base/files");
        if directory {
            fs::rename(&files, catalog.join("secret")).unwrap();
            symlink(catalog.join("secret"), &files).unwrap();
        } else {
            fs::remove_file(files.join("target.md")).unwrap();
            fs::write(catalog.join("secret"), b"SECRET").unwrap();
            symlink(catalog.join("secret"), files.join("target.md")).unwrap();
        }
        let result = checked_preview(&scratch.0, &catalog, &["custom".into()]);
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
        catalog.join("bootstrap/base.toml"),
        catalog.join("secret.toml"),
    )
    .unwrap();
    symlink(
        catalog.join("secret.toml"),
        catalog.join("bootstrap/base.toml"),
    )
    .unwrap();
    assert!(checked_preview(&scratch.0, &catalog, &["custom".into()]).is_err());
    assert!(!scratch.0.join(".maestro").exists());
}

#[test]
fn template_size_at_limit_is_complete_and_one_past_is_refused() {
    let scratch = Scratch::new();
    let catalog = scratch.catalog("base");
    let limit = usize::try_from(Limits::PRODUCTION.source_file_bytes).unwrap();
    let bytes = vec![b'x'; limit];
    let template = catalog.join("bootstrap/base/files/target.md");
    fs::write(&template, &bytes).unwrap();
    let manifest = catalog.join("bootstrap/base.toml");
    let text = fs::read_to_string(&manifest)
        .unwrap()
        .replace(&digest(b"inert"), &digest(&bytes));
    fs::write(manifest, text).unwrap();
    let provider = checked_port(&catalog).unwrap();
    let selected = ["custom".into()];
    let presets = provider.resolve(&selected).unwrap();
    assert_eq!(presets[0].files["target.md"], bytes);
    fs::write(&template, vec![b'x'; limit + 1]).unwrap();
    let error = checked_preview(&scratch.0, &catalog, &selected).unwrap_err();
    assert!(error.contains("bootstrap/base/files/target.md"), "{error}");
    assert!(error.contains("larger than"), "{error}");
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
    assert!(!scratch.0.join("target.md").exists());
    assert!(!scratch.0.join(".maestro").exists());
    assert!(!scratch.0.join(".maestro-files").exists());
}

#[test]
fn manifest_size_at_limit_parses_and_one_past_is_refused() {
    let scratch = Scratch::new();
    let catalog = scratch.catalog("base");
    let path = catalog.join("bootstrap/base.toml");
    let mut bytes = fs::read(&path).unwrap();
    bytes.resize(
        usize::try_from(Limits::PRODUCTION.source_file_bytes).unwrap(),
        b' ',
    );
    fs::write(&path, &bytes).unwrap();
    let provider = checked_port(&catalog).unwrap();
    assert!(provider.resolve(&["custom".into()]).is_ok());
    bytes.push(b' ');
    fs::write(&path, bytes).unwrap();
    let error = checked_port(&catalog).unwrap_err();
    assert!(error.contains("larger than"), "{error}");
}

#[test]
fn new_inventory_is_manifest_data_not_a_code_change() {
    let scratch = Scratch::new();
    let catalog = scratch.catalog("python");
    let preview = checked_preview(&scratch.0, &catalog, &["custom".into()]).unwrap();
    apply(&scratch.0, &preview).unwrap();
    assert_eq!(fs::read(scratch.0.join("target.md")).unwrap(), b"inert");
}

#[test]
fn different_presets_shipping_the_same_path_are_refused() {
    let scratch = Scratch::new();
    let catalog = scratch.catalog("base");
    let manifest = fs::read_to_string(catalog.join("bootstrap/base.toml"))
        .unwrap()
        .replace(r#"name = "base""#, r#"name = "second""#);
    fs::write(catalog.join("bootstrap/second.toml"), manifest).unwrap();
    fs::create_dir_all(catalog.join("bootstrap/second/files")).unwrap();
    fs::copy(
        catalog.join("bootstrap/base/files/target.md"),
        catalog.join("bootstrap/second/files/target.md"),
    )
    .unwrap();
    let preset = fs::read_to_string(catalog.join("presets/custom.toml"))
        .unwrap()
        .replace("custom", "second")
        .replace("common/base", "common/second");
    fs::write(catalog.join("presets/second.toml"), preset).unwrap();
    let error =
        checked_preview(&scratch.0, &catalog, &["custom".into(), "second".into()]).unwrap_err();
    assert!(error.contains("file collision: target.md"), "{error}");
    assert!(!scratch.0.join(".maestro").exists());
}

#[test]
fn base_only_descriptor_has_exact_authoring_keys_and_lock_reference() {
    let scratch = Scratch::new();
    let catalog = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/catalog/bootstrap/owner-local")
        .canonicalize()
        .unwrap();
    let preview = checked_preview(&scratch.0, &catalog, &["base".into()]).unwrap();
    assert!(!scratch.0.join(".maestro").exists());
    apply(&scratch.0, &preview).unwrap();
    let descriptor: toml::Table =
        toml::from_str(&fs::read_to_string(scratch.0.join(".maestro/project.toml")).unwrap())
            .unwrap();
    assert_eq!(
        descriptor.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "areas",
            "capabilities",
            "context_files",
            "lock",
            "mode",
            "presets",
            "schema"
        ]
    );
    assert_eq!(descriptor["schema"].as_str(), Some("maestro-project/2"));
    assert_eq!(descriptor["mode"].as_str(), Some("authoring"));
    assert_eq!(
        descriptor["lock"].as_str(),
        Some(".maestro/authoring.lock.json")
    );
    assert_eq!(
        descriptor["presets"].as_array().unwrap(),
        &[toml::Value::String("preset:base".into())]
    );
    assert_eq!(
        fs::read(scratch.0.join(".github/copilot-instructions.md")).unwrap(),
        fs::read(catalog.join("bootstrap/base/files/instructions.md")).unwrap()
    );
    assert!(!scratch.0.join(".maestro/recipes.json").exists());
}

#[test]
fn authoring_lock_binds_every_generated_file_and_source() {
    use sha2::{Digest as _, Sha256};
    use std::fmt::Write as _;
    let scratch = Scratch::new();
    let catalog = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/catalog/bootstrap/owner-local")
        .canonicalize()
        .unwrap();
    let preview = checked_preview(&scratch.0, &catalog, &["base".into(), "rust".into()]).unwrap();
    apply(&scratch.0, &preview).unwrap();
    let lock: serde_json::Value =
        serde_json::from_slice(&fs::read(scratch.0.join(".maestro/authoring.lock.json")).unwrap())
            .unwrap();
    assert_eq!(lock["schema"], "maestro-authoring-lock/3");
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
                "bootstrap/base.toml",
                "bootstrap/base/files/instructions.md",
                "core/agents/maestro.agent.md",
                "core/agents/maestro.maestro.toml",
                "core/package.toml",
                "languages/rust/bootstrap/starter.toml",
                "languages/rust/bootstrap/starter/files/recipes.json",
                "languages/rust/package.toml",
                "package.toml",
                "presets/base.toml",
                "presets/rust.toml",
                "standards/quality/package.toml",
                "standards/security/package.toml",
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
    let provider = checked_port(&catalog).unwrap();
    let names = ["custom".into()];
    let first = preview(&scratch.0, &provider, &names).unwrap();
    apply(&scratch.0, &first).unwrap();
    let replay = preview(&scratch.0, &provider, &names).unwrap();
    assert!(replay.plan.is_applied());
    remove(&scratch.0, replay.plan.id()).unwrap();
    assert!(apply_with_failure(&scratch.0, &replay.plan, Some(1)).is_err());
    assert!(!scratch.0.join("target.md").exists());
    recover(&scratch.0, replay.plan.id()).unwrap();
    assert_eq!(fs::read(scratch.0.join("target.md")).unwrap(), b"inert");
    assert!(
        preview(&scratch.0, &provider, &names)
            .unwrap()
            .plan
            .is_applied()
    );
}

#[test]
fn path_tool_candidates_cover_unix_and_windows_extensions() {
    use super::super::project::tool_candidates;
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
fn inventories_and_tools_are_single_top_level_names() {
    let scratch = Scratch::new();
    let catalog = scratch.catalog("base");
    for tool in ["../tool", "tool --install"] {
        let manifest = catalog.join("bootstrap/base.toml");
        let original = fs::read_to_string(&manifest).unwrap();
        fs::write(
            &manifest,
            original.replace("tools = []", &format!("tools = [\"{tool}\"]")),
        )
        .unwrap();
        assert!(checked_port(&catalog).is_err(), "accepted {tool}");
        fs::write(manifest, original).unwrap();
    }
    assert!(!scratch.0.join(".maestro").exists());
}

#[test]
fn old_authoring_lock_requires_fresh_preview() {
    let scratch = Scratch::new();
    fs::create_dir_all(scratch.0.join(".maestro")).unwrap();
    fs::write(
        scratch.0.join(".maestro/authoring.lock.json"),
        br#"{"schema":"maestro-authoring-lock/1","files":[],"sources":[]}"#,
    )
    .unwrap();
    let result = preview(&scratch.0, &Presets, &["base".into()]);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(
        error.contains("maestro-authoring-lock/1") && error.contains("fresh preview"),
        "{error}"
    );
}

#[test]
fn authoring_lock_read_keeps_source_byte_bound() {
    let scratch = Scratch::new();
    fs::create_dir_all(scratch.0.join(".maestro")).unwrap();
    let mut bytes = br#"{"schema":"maestro-authoring-lock/3"}"#.to_vec();
    bytes.resize(
        usize::try_from(Limits::PRODUCTION.source_file_bytes).unwrap() + 1,
        b' ',
    );
    fs::write(scratch.0.join(".maestro/authoring.lock.json"), bytes).unwrap();
    let result = preview(&scratch.0, &Presets, &["base".into()]);
    assert!(
        result
            .unwrap_err()
            .contains("file is larger than 1048576 bytes")
    );
}
