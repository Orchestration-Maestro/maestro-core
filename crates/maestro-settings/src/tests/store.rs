//! The file adapter: bounded reads naming the file.

use crate::{FileLayers, LayerName, LayerSource, MAX_FILE_BYTES, Registry, USER_FILE, Value};
use maestro_test_scratch::scratch_directory;
use std::{fs, path::PathBuf};

/// A scratch directory, removed when dropped.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

/// A new scratch directory.
fn scratch() -> Scratch {
    Scratch(scratch_directory().unwrap())
}

#[test]
fn file_layers_read_each_present_file_and_skip_a_missing_one() {
    let scratch = scratch();
    let registry = Registry::built_in().unwrap();
    let missing = FileLayers::new(&scratch.0, Some(scratch.0.join("project.toml")));
    let layers = missing.layers(&registry).unwrap();
    assert_eq!(layers.user, None);
    assert_eq!(layers.project, None);

    fs::write(
        scratch.0.join(USER_FILE),
        "schema = \"maestro-preferences/1\"\ntone = \"brief\"\n",
    )
    .unwrap();
    fs::write(
        scratch.0.join("project.toml"),
        "schema = \"maestro-preferences/1\"\nsearch.k = 3\n",
    )
    .unwrap();
    let layers = missing.layers(&registry).unwrap();
    let (user_path, user) = layers.user.unwrap();
    assert_eq!(user_path, scratch.0.join(USER_FILE));
    assert_eq!(user.get("tone"), Some(&Value::Text("brief".to_owned())));
    let (project_path, project) = layers.project.unwrap();
    assert_eq!(project_path, scratch.0.join("project.toml"));
    assert_eq!(project.get("search.k"), Some(&Value::Integer(3)));
    assert_eq!(
        missing.path_of(LayerName::User),
        Some(scratch.0.join(USER_FILE))
    );
    assert_eq!(
        missing.path_of(LayerName::Project),
        Some(scratch.0.join("project.toml"))
    );
    assert_eq!(
        FileLayers::new(&scratch.0, None).path_of(LayerName::Project),
        None
    );
}

#[test]
fn file_layers_refuse_a_file_naming_it() {
    let scratch = scratch();
    let registry = Registry::built_in().unwrap();
    fs::write(
        scratch.0.join(USER_FILE),
        "schema = \"maestro-preferences/1\"\n[access]\nread = []\n",
    )
    .unwrap();
    let error = FileLayers::new(&scratch.0, None)
        .layers(&registry)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        format!(
            "{}: unknown key \"access\"",
            scratch.0.join(USER_FILE).display()
        )
    );
}

#[test]
fn file_layers_read_no_further_than_a_preferences_file_may_hold() {
    let scratch = scratch();
    let registry = Registry::built_in().unwrap();
    let path = scratch.0.join(USER_FILE);
    let layers = FileLayers::new(&scratch.0, None);
    let refusal = || layers.layers(&registry).unwrap_err().to_string();
    fs::write(&path, vec![b' '; MAX_FILE_BYTES + 1]).unwrap();
    assert_eq!(
        refusal(),
        format!(
            "{}: the file holds {} bytes, more than the {MAX_FILE_BYTES} a preferences file may",
            path.display(),
            MAX_FILE_BYTES + 1
        )
    );
    fs::write(&path, vec![b' '; MAX_FILE_BYTES]).unwrap();
    assert_eq!(
        refusal(),
        format!(
            "{}: the file has no schema = \"maestro-preferences/1\" line",
            path.display()
        )
    );
    fs::write(&path, [0xff, 0xfe]).unwrap();
    assert!(
        refusal().starts_with(&format!("cannot read {}: ", path.display())),
        "{}",
        refusal()
    );
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(layers.layers(&registry).is_err());
}
