//! Real planted files on every host, with no mocked owner/write metadata.
use crate::{
    limits::Limits,
    settings::{
        Layer as PreferenceLayer, NoWorkspaceTrust, SessionPreferences, WorkspacePreferences,
        WorkspaceTrust, resolve,
    },
};
use maestro_settings::{Registry, Value, parse_flags};
use maestro_test_scratch::scratch_directory;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// A disposable home, with its user file separate from the workspace tree.
pub(super) struct Home(pub(super) PathBuf);
impl Home {
    pub(super) fn new() -> Self {
        Self(scratch_directory().unwrap())
    }
    pub(super) fn plant(&self, directory: &str, text: &str) -> PathBuf {
        let path = self.0.join(directory);
        fs::create_dir_all(path.join(".maestro")).unwrap();
        fs::write(path.join(".maestro/config.toml"), text).unwrap();
        path
    }
    pub(super) fn load(&self, start: Option<&Path>) -> Result<SessionPreferences, String> {
        SessionPreferences::load(
            &self.0.join("user"),
            start,
            Some(&self.0),
            &NoWorkspaceTrust,
            &Limits::PRODUCTION,
        )
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

/// Synthetic approval supplied through the authority port, not preference content.
struct Approved(PathBuf);
impl WorkspaceTrust for Approved {
    fn containing_root(&self, start: &Path) -> Option<PathBuf> {
        start.starts_with(&self.0).then(|| self.0.clone())
    }
}

#[test]
fn home_boundary_nearest_missing_and_ignored_ancestors() {
    let home = Home::new();
    let root = home.plant(
        "project",
        "schema = 'maestro-preferences/1'\ntone = 'brief'",
    );
    let nested = home.plant(
        "project/nested",
        "schema = 'maestro-preferences/1'\nlanguage = 'JA'",
    );
    // Ignored ancestors are not parsed or merged, even when malformed.
    fs::write(root.join(".maestro/config.toml"), "invalid ancestor").unwrap();
    let snapshot = home.load(Some(&nested)).unwrap();
    let registry = Registry::built_in().unwrap();
    let layers = snapshot.layers(&registry, &Limits::PRODUCTION).unwrap();
    let selected = resolve(
        &registry,
        &maestro_settings::resolve(&registry, &layers, &[]),
    );
    assert_eq!(selected.text("language"), Some("ja"));
    assert_eq!(selected.text("tone"), Some("normal"));
    assert_eq!(
        snapshot.discovery.file,
        Some(nested.join(".maestro/config.toml").canonicalize().unwrap())
    );
    fs::remove_file(nested.join(".maestro/config.toml")).unwrap();
    assert!(home.load(Some(&nested)).unwrap_err().contains("TOML"));
    fs::remove_file(root.join(".maestro/config.toml")).unwrap();
    assert!(home.load(Some(&nested)).unwrap().discovery.file.is_none());
}

#[test]
fn external_roots_require_port_approval_and_never_climb_above_it() {
    let home = Home::new();
    let external = Home::new();
    let root = external.plant(
        "approved",
        "schema = 'maestro-preferences/1'\ntone = 'brief'",
    );
    external.plant("", "invalid ancestor outside approval");
    let denied = home.load(Some(&root)).unwrap();
    assert!(denied.discovery.file.is_none());
    assert!(denied.discovery.note.unwrap().contains("maestro trust add"));
    let approved = Approved(root.canonicalize().unwrap());
    let snapshot = SessionPreferences::load(
        &home.0.join("user"),
        Some(&root),
        Some(&home.0),
        &approved,
        &Limits::PRODUCTION,
    )
    .unwrap();
    assert_eq!(
        snapshot.discovery.file,
        Some(root.join(".maestro/config.toml").canonicalize().unwrap())
    );
    fs::remove_file(root.join(".maestro/config.toml")).unwrap();
    let empty = SessionPreferences::load(
        &home.0.join("user"),
        Some(&root),
        Some(&home.0),
        &approved,
        &Limits::PRODUCTION,
    )
    .unwrap();
    assert!(empty.discovery.file.is_none());
}

#[test]
fn four_layers_preserve_absent_language_and_intersect_update_budget_ceilings() {
    let home = Home::new();
    let root = home.plant("project", "schema = 'maestro-preferences/1'\n");
    let registry = Registry::built_in().unwrap();
    let empty = home
        .load(Some(&root))
        .unwrap()
        .layers(&registry, &Limits::PRODUCTION)
        .unwrap();
    assert_eq!(
        resolve(
            &registry,
            &maestro_settings::resolve(&registry, &empty, &[])
        )
        .text("language"),
        Some("auto")
    );
    fs::create_dir(home.0.join("user")).unwrap();
    fs::write(
        home.0.join("user/preferences.toml"),
        concat!(
            "schema = 'maestro-preferences/1'\n",
            "language = 'es'\n",
            "tone = 'brief'\n",
            "updates = 'off'\n",
            "ask.output_tokens = 100",
        ),
    )
    .unwrap();
    fs::write(
        root.join(".maestro/config.toml"),
        concat!(
            "schema = 'maestro-preferences/1'\n",
            "language = 'fr'\n",
            "updates = 'auto'\n",
            "ask.output_tokens = 80",
        ),
    )
    .unwrap();
    let snapshot = home.load(Some(&root)).unwrap();
    let layers = snapshot.layers(&registry, &Limits::PRODUCTION).unwrap();
    let flags = parse_flags(
        &registry,
        &[
            "language=ja".into(),
            "updates=propose".into(),
            "ask.output_tokens=200".into(),
        ],
    )
    .unwrap();
    let selected = resolve(
        &registry,
        &maestro_settings::resolve(&registry, &layers, &flags),
    );
    assert_eq!(selected.text("language"), Some("ja"));
    assert_eq!(selected.text("tone"), Some("brief"));
    assert_eq!(selected.text("updates"), Some("off"));
    assert_eq!(selected.integer("ask.output_tokens"), Some(80));
    assert_eq!(
        selected.get("language").unwrap().as_ref().unwrap().source(),
        "flag"
    );
    assert_eq!(
        selected
            .get("language")
            .unwrap()
            .as_ref()
            .unwrap()
            .overridden(),
        &[
            (PreferenceLayer::Workspace, Value::Text("fr".into())),
            (PreferenceLayer::User, Value::Text("es".into()))
        ]
    );
    assert!(
        selected
            .diagnostics()
            .iter()
            .any(|entry| entry.message.contains("workspace update widening"))
    );
    fs::write(
        home.0.join("user/preferences.toml"),
        "schema = 'maestro-preferences/1'\nupdates = 'propose'",
    )
    .unwrap();
    let narrowed = home
        .load(Some(&root))
        .unwrap()
        .layers(&registry, &Limits::PRODUCTION)
        .unwrap();
    assert_eq!(
        resolve(
            &registry,
            &maestro_settings::resolve(&registry, &narrowed, &[])
        )
        .text("updates"),
        Some("propose")
    );
}

#[test]
fn all_present_safe_files_validate_even_when_flags_mask_keys_and_snapshots_do_not_watch() {
    let home = Home::new();
    let root = home.plant(
        "project",
        "schema = 'maestro-preferences/1'\ntone = 'brief'",
    );
    let snapshot = home.load(Some(&root)).unwrap();
    for text in [
        "schema = 'maestro-preferences/1'\nunknown = true",
        "schema = 'maestro-preferences/1'\n[access]\nread = []",
    ] {
        fs::write(root.join(".maestro/config.toml"), text).unwrap();
        assert!(home.load(Some(&root)).is_err());
    }
    let registry = Registry::built_in().unwrap();
    let fixed = snapshot.layers(&registry, &Limits::PRODUCTION).unwrap();
    assert_eq!(
        fixed.project.unwrap().1.get("tone"),
        Some(&Value::Text("brief".into()))
    );
    fs::create_dir(home.0.join("user")).unwrap();
    fs::write(home.0.join("user/preferences.toml"), "unknown = true").unwrap();
    fs::write(
        root.join(".maestro/config.toml"),
        "schema = 'maestro-preferences/1'\ntone = 'normal'",
    )
    .unwrap();
    assert!(home.load(Some(&root)).is_err());
}

#[test]
fn unsafe_valid_and_invalid_files_or_directories_are_skipped_not_parsed() {
    for directory in [false, true] {
        for text in [
            "invalid planted file",
            "schema = 'maestro-preferences/1'\ntone = 'detailed'",
        ] {
            let home = Home::new();
            let root = home.plant(
                "project",
                "schema = 'maestro-preferences/1'\ntone = 'brief'",
            );
            let nested = home.plant("project/nested", text);
            let target = if directory {
                nested.join(".maestro")
            } else {
                nested.join(".maestro/config.toml")
            };
            other_write(&target);
            let snapshot = home.load(Some(&nested)).unwrap();
            assert_eq!(
                snapshot.discovery.file,
                Some(root.join(".maestro/config.toml").canonicalize().unwrap())
            );
            assert_eq!(snapshot.discovery.skipped.len(), 1);
            assert!(snapshot.discovery.skipped[0].contains("writable"));
        }
    }
}

#[test]
fn foreign_owner_candidates_skip_to_safe_current_user_neighbor() {
    for directory in [false, true] {
        for text in [
            "invalid planted file",
            "schema = 'maestro-preferences/1'\ntone = 'detailed'",
        ] {
            let home = Home::new();
            let root = home.plant(
                "project",
                "schema = 'maestro-preferences/1'\ntone = 'brief'",
            );
            let nested = home.plant("project/nested", text);
            let target = if directory {
                nested.join(".maestro")
            } else {
                nested.join(".maestro/config.toml")
            };
            foreign_owner(&target);
            let snapshot = home.load(Some(&nested)).unwrap();
            assert_eq!(
                snapshot.discovery.file,
                Some(root.join(".maestro/config.toml").canonicalize().unwrap())
            );
            assert!(snapshot.discovery.skipped[0].contains("foreign-owned"));
            restore_owner(&target);
        }
    }
}

#[cfg(unix)]
fn other_write(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if path.is_dir() { 0o777 } else { 0o666 }),
    )
    .unwrap();
}
#[cfg(windows)]
fn other_write(path: &Path) {
    let status = Command::new("icacls")
        .arg(path)
        .args(["/grant", "*S-1-1-0:(W)"])
        .status()
        .unwrap();
    assert!(status.success());
}
#[cfg(unix)]
fn foreign_owner(path: &Path) {
    let status = Command::new("sudo")
        .args(["-n", "chown", "0"])
        .arg(path)
        .status()
        .unwrap();
    assert!(
        status.success(),
        "real owner probe requires CI host's passwordless sudo"
    );
}
#[cfg(unix)]
fn restore_owner(path: &Path) {
    let output = Command::new("id").arg("-u").output().unwrap();
    let user = String::from_utf8(output.stdout).unwrap();
    assert!(
        Command::new("sudo")
            .args(["-n", "chown", user.trim()])
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
}
#[cfg(windows)]
fn foreign_owner(path: &Path) {
    assert!(
        Command::new("icacls")
            .arg(path)
            .args(["/setowner", "*S-1-5-32-544"])
            .status()
            .unwrap()
            .success()
    );
}
#[cfg(windows)]
fn restore_owner(path: &Path) {
    let user = Command::new("whoami").output().unwrap();
    let user = String::from_utf8(user.stdout).unwrap();
    assert!(
        Command::new("icacls")
            .arg(path)
            .args(["/setowner", user.trim()])
            .status()
            .unwrap()
            .success()
    );
}

#[cfg(unix)]
#[test]
fn unreadable_candidates_and_symlink_files_or_directories_skip_to_safe_neighbor() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    let home = Home::new();
    let root = home.plant(
        "project",
        "schema = 'maestro-preferences/1'\ntone = 'brief'",
    );
    let nested = home.plant("project/nested", "invalid unreadable planted file");
    let file = nested.join(".maestro/config.toml");
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
    let snapshot = home.load(Some(&nested)).unwrap();
    assert_eq!(snapshot.discovery.skipped.len(), 1);
    assert_eq!(
        snapshot.discovery.file,
        Some(root.join(".maestro/config.toml").canonicalize().unwrap())
    );
    fs::remove_file(&file).unwrap();
    symlink(root.join(".maestro/config.toml"), &file).unwrap();
    assert_eq!(home.load(Some(&nested)).unwrap().discovery.skipped.len(), 1);
    fs::remove_file(&file).unwrap();
    fs::remove_dir(nested.join(".maestro")).unwrap();
    symlink(root.join(".maestro"), nested.join(".maestro")).unwrap();
    let linked = home.load(Some(&nested)).unwrap();
    assert_eq!(linked.discovery.skipped.len(), 1);
    assert_eq!(
        linked.discovery.file,
        Some(root.join(".maestro/config.toml").canonicalize().unwrap())
    );
}

#[test]
fn selected_safe_oversize_or_non_utf8_file_refuses_instead_of_falling_through() {
    let home = Home::new();
    let root = home.plant("project", "schema = 'maestro-preferences/1'");
    let mut limits = Limits::PRODUCTION;
    limits.source_file_bytes = 5;
    assert!(
        SessionPreferences::load(
            &home.0.join("user"),
            Some(&root),
            Some(&home.0),
            &NoWorkspaceTrust,
            &limits
        )
        .is_err()
    );
    fs::write(root.join(".maestro/config.toml"), [0xff]).unwrap();
    assert!(home.load(Some(&root)).is_err());
}

#[test]
fn tmp_and_drive_style_ancestors_outside_home_are_never_read() {
    for parent in ["tmp", "mnt/c"] {
        let outer = Home::new();
        let ignored = outer.plant(parent, "invalid planted ancestor");
        let home = ignored.join("home");
        fs::create_dir(&home).unwrap();
        let snapshot = SessionPreferences::load(
            &home.join("user"),
            Some(&home),
            Some(&home),
            &NoWorkspaceTrust,
            &Limits::PRODUCTION,
        )
        .unwrap();
        assert!(snapshot.discovery.file.is_none());
        assert!(snapshot.discovery.skipped.is_empty());
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "real bind mount requires the Ubuntu CI runner's passwordless sudo"]
fn same_filesystem_bind_mount_root_is_never_selected() {
    use std::os::unix::fs::MetadataExt as _;
    let home = Home::new();
    let safe = home.plant("", "schema = 'maestro-preferences/1'\ntone = 'normal'");
    let source = home.plant("source", "invalid bind-mount root");
    let mounted = home.0.join("mounted");
    fs::create_dir(&mounted).unwrap();
    assert!(
        Command::new("sudo")
            .args(["-n", "mount", "--bind"])
            .arg(&source)
            .arg(&mounted)
            .status()
            .unwrap()
            .success()
    );
    // Capture results before unmounting, but assert only after cleanup.
    let mounted_device = fs::metadata(&mounted).map(|metadata| metadata.dev());
    let parent_device = fs::metadata(&home.0).map(|metadata| metadata.dev());
    let loaded = home.load(Some(&mounted));
    assert!(
        Command::new("sudo")
            .args(["-n", "umount"])
            .arg(&mounted)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(mounted_device.unwrap(), parent_device.unwrap());
    let snapshot = loaded.unwrap();
    assert_eq!(
        snapshot.discovery.file,
        Some(safe.join(".maestro/config.toml").canonicalize().unwrap())
    );
    assert_eq!(snapshot.discovery.skipped.len(), 1);
    assert!(snapshot.discovery.skipped[0].contains("mount or drive root"));
}
