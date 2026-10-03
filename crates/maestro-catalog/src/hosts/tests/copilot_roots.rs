//! Canonical-root boundaries, missing discovery bindings and native filename decoding.
use super::{
    super::copilot::{Copilot, NativeProjection as _},
    copilot::Fixture,
};
use crate::files::tests::support::with_trust;
#[cfg(unix)]
use std::io::ErrorKind;
use std::{fs, path::MAIN_SEPARATOR_STR};

#[test]
fn hosts_copilot_missing_home_is_rechecked_for_new_shadows_before_apply() {
    let fixture = Fixture::new();
    let snapshot = fixture.snapshot();
    let home = fixture.root.join("missing/deeper");
    let adapter = Copilot::from_home(Some(home.clone())).unwrap();
    with_trust(&fixture.project, |trust| {
        let preview = adapter
            .preview(&fixture.project, &snapshot, false, trust)
            .unwrap();
        fs::create_dir_all(home.join("agents")).unwrap();
        fs::write(
            home.join("agents/new.agent.md"),
            b"---\nname: maestro\n---\n",
        )
        .unwrap();
        let error = preview.apply(&fixture.project, trust).unwrap_err();
        assert!(error.to_string().contains("shadows the projection"));
        assert!(!fixture.project.join(".github").exists());
    });
}

#[cfg(unix)]
#[test]
fn hosts_copilot_missing_home_resolves_symlinked_ancestor() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let snapshot = fixture.snapshot();
    let real = fixture.root.join("real");
    fs::create_dir(&real).unwrap();
    let alias = fixture.root.join("alias");
    symlink(&real, &alias).unwrap();
    let adapter = Copilot::from_home(Some(alias.join("missing/deeper"))).unwrap();
    with_trust(&fixture.project, |trust| {
        let preview = adapter
            .preview(&fixture.project, &snapshot, false, trust)
            .unwrap();
        assert_eq!(
            preview.user_agents,
            Some(real.canonicalize().unwrap().join("missing/deeper/agents"))
        );
    });
}

#[test]
fn hosts_copilot_missing_home_refuses_non_normal_tail() {
    let fixture = Fixture::new();
    for tail in ["missing/../other", "missing/./other"] {
        // Verbatim Windows PathBuf::join normalizes dots; preserve the actual caller input.
        let mut named = fixture.root.as_os_str().to_os_string();
        named.push(MAIN_SEPARATOR_STR);
        named.push(tail.replace('/', MAIN_SEPARATOR_STR));
        let error = Copilot::from_home(Some(named.into())).unwrap_err();
        assert!(error.to_string().contains("normal names only"), "{error}");
    }
}

#[cfg(unix)]
#[test]
fn hosts_copilot_home_resolution_propagates_not_a_directory() {
    let fixture = Fixture::new();
    let file = fixture.root.join("not-a-directory");
    fs::write(&file, b"not a directory").unwrap();
    let error = Copilot::from_home(Some(file.join("missing"))).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NotADirectory);
}

#[cfg(unix)]
#[test]
fn hosts_copilot_missing_home_keeps_backslashes_in_normal_unix_names() {
    let fixture = Fixture::new();
    let snapshot = fixture.snapshot();
    let name = r"literal\.\missing";
    let home = fixture.root.join(name);
    let adapter = Copilot::from_home(Some(home.clone())).unwrap();
    with_trust(&fixture.project, |trust| {
        let preview = adapter
            .preview(&fixture.project, &snapshot, false, trust)
            .unwrap();
        assert_eq!(
            preview.user_agents,
            Some(
                fixture
                    .root
                    .canonicalize()
                    .unwrap()
                    .join(name)
                    .join("agents")
            )
        );
    });
}

#[cfg(unix)]
#[test]
fn hosts_copilot_preview_refuses_noncanonical_target_before_opening() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let alias = fixture.root.join("alias");
    symlink(&fixture.root, &alias).unwrap();
    let snapshot = fixture.snapshot();
    let adapter = Copilot::new(None);
    with_trust(&fixture.project, |trust| {
        let root = alias.join("project");
        let error = adapter.preview(&root, &snapshot, false, trust).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("root is not canonical: {}", root.display())
        );
        assert!(
            adapter
                .preview(&fixture.project, &snapshot, false, trust)
                .is_ok()
        );
        assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
    });
}

#[cfg(unix)]
#[test]
fn hosts_copilot_new_read_root_requires_canonical_input_at_preview() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let alias = fixture.root.join("alias");
    symlink(&fixture.root, &alias).unwrap();
    let user = fixture.root.join("user-agents");
    fs::create_dir(&user).unwrap();
    let named = alias.join("user-agents");
    let snapshot = fixture.snapshot();
    with_trust(&fixture.project, |trust| {
        let error = Copilot::new(Some(named.clone()))
            .preview(&fixture.project, &snapshot, false, trust)
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("root is not canonical: {}", named.display())
        );
        assert!(
            Copilot::new(Some(user))
                .preview(&fixture.project, &snapshot, false, trust)
                .is_ok()
        );
        assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
    });
}

#[test]
fn hosts_copilot_profile_filename_classifies_raw_native_names() {
    use super::super::copilot::profile_filename;
    use std::ffi::{OsStr, OsString};
    #[cfg(unix)]
    let invalid = {
        use std::os::unix::ffi::OsStringExt as _;
        OsString::from_vec(b"\xff.agent.md".to_vec())
    };
    #[cfg(windows)]
    let invalid = {
        use std::os::windows::ffi::OsStringExt as _;
        OsString::from_wide(&[0xd800, 46, 97, 103, 101, 110, 116, 46, 109, 100])
    };
    assert!(
        profile_filename(&invalid)
            .unwrap_err()
            .to_string()
            .contains("not UTF-8")
    );
    assert_eq!(
        profile_filename(OsStr::new("neighbour.agent.md")).unwrap(),
        Some("neighbour.agent.md")
    );
    assert_eq!(profile_filename(OsStr::new("neighbour.md")).unwrap(), None);
}
