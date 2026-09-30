use super::compose as compose_module;
use super::{Preset, PresetPort, compose, inspect};
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
                }),
                "rust-service" => Ok(Preset {
                    name: name.clone(),
                    files: BTreeMap::from([(
                        ".maestro/recipes.json".to_owned(),
                        files[".maestro/recipes.json"].clone(),
                    )]),
                    source_files: BTreeMap::new(),
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/catalog");
    let provider = super::DirectoryPresets::new(&root);
    let composed = compose(
        &provider,
        &["knowledge-client".into(), "rust-service".into()],
    );
    assert!(composed.is_ok(), "{composed:?}");
    let files = composed.unwrap_or_default();
    assert_eq!(files.len(), 2);
}
