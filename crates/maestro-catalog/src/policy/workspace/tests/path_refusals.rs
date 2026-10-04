//! Failure and alias cases sharing the real filesystem path fixtures.
use super::{
    Access, Directory, Environment, Fixture, Path, Roots, TrustBoundaries, directory_link, fs,
    literal_suffix,
};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::Component;

#[test]
fn missing_base_and_inaccessible_file_refuse_beside_readable_leaf() {
    let fixture = Fixture::new();
    fixture.file("project/plain");
    fixture.file("project/locked");
    let base = literal_suffix(&fixture.project, "..");
    assert!(base.components().any(|part| part == Component::ParentDir));
    for access in [Access::Read, Access::Write] {
        assert!(
            fixture
                .trust()
                .authorize(&fixture.home.join("absent"), Path::new("new"), access)
                .is_err()
        );
        assert!(
            fixture
                .trust()
                .authorize(&base, Path::new("project/plain"), access)
                .is_err()
        );
    }
    #[cfg(unix)]
    {
        fs::set_permissions(
            fixture.project.join("locked"),
            fs::Permissions::from_mode(0o0),
        )
        .unwrap();
        for access in [Access::Read, Access::Write] {
            assert!(
                fixture
                    .trust()
                    .authorize(&fixture.project, Path::new("locked"), access)
                    .is_err()
            );
            assert!(
                fixture
                    .trust()
                    .authorize(&fixture.project, Path::new("plain"), access)
                    .is_ok()
            );
        }
        fs::set_permissions(
            fixture.project.join("locked"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("plain"), Access::Read)
            .is_ok()
    );
}

#[test]
fn grants_cannot_promote_home_internal_or_drive_roots_into_write_authority() {
    let mut fixture = Fixture::new();
    fixture.file("project/plain");
    for root in [
        fixture.home.clone(),
        fixture.home.join("kernel"),
        fixture.home.ancestors().last().unwrap().to_path_buf(),
        fixture.outside.clone(),
    ] {
        fixture.roots = Roots(vec![root]);
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new("plain"), Access::Write)
                .is_err()
        );
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new("plain"), Access::Read)
                .is_ok()
        );
    }
    fixture.roots = Roots(vec![fixture.project.clone()]);
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("plain"), Access::Write)
            .is_ok()
    );
}

#[test]
fn canonical_secret_alias_and_internal_alias_stay_denied_beside_neighbour() {
    let mut fixture = Fixture::new();
    fixture.file("project/keys/key");
    fixture.file("project/notes/key");
    directory_link(&fixture.project.join("keys"), &fixture.home.join(".ssh"));
    fixture.boundaries =
        TrustBoundaries::with_environment(&fixture.home, &[], &Environment::default()).unwrap();
    for access in [Access::Read, Access::Write] {
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new("keys/key"), access)
                .is_err()
        );
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new("notes/key"), access)
                .is_ok()
        );
    }
    // No workspace-controlled content can replace the compiled deny source.
    fs::write(
        fixture.project.join("secret-paths.json"),
        b"{\"paths\":[],\"names\":[]}",
    )
    .unwrap();
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("keys/key"), Access::Read)
            .is_err()
    );
    drop(Directory::open_canonical(&fixture.project).unwrap());
}

#[test]
fn relative_xdg_bindings_use_home_fallbacks_not_working_directory() {
    let mut fixture = Fixture::new();
    let mut environment = Environment::default();
    environment.xdg_config_home = Some("relative-config".into());
    environment.xdg_data_home = Some("relative-data".into());
    fixture.file("project/user-home/.config/gcloud/credentials");
    fixture.file("project/user-home/.local/share/keyrings/private");
    fixture.file("project/user-home/.config/notes/plain");
    fixture.boundaries =
        TrustBoundaries::with_environment(&fixture.project.join("user-home"), &[], &environment)
            .unwrap();
    for secret in [
        "user-home/.config/gcloud/credentials",
        "user-home/.local/share/keyrings/private",
    ] {
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new(secret), Access::Read)
                .is_err()
        );
    }
    assert!(
        fixture
            .trust()
            .authorize(
                &fixture.project,
                Path::new("user-home/.config/notes/plain"),
                Access::Read
            )
            .is_ok()
    );
}

#[cfg(unix)]
#[test]
fn unverifiable_secret_mapping_and_non_utf8_names_fail_closed() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt as _, path::PathBuf};
    let fixture = Fixture::new();
    fixture.file(".kube/config");
    fs::set_permissions(fixture.home.join(".kube"), fs::Permissions::from_mode(0o0)).unwrap();
    let mapping = TrustBoundaries::with_environment(&fixture.home, &[], &Environment::default());
    fs::set_permissions(
        fixture.home.join(".kube"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    assert!(mapping.is_err());
    super::symlink(fixture.home.join(".ssh"), fixture.home.join(".ssh")).unwrap();
    assert!(
        TrustBoundaries::with_environment(&fixture.home, &[], &Environment::default()).is_err()
    );
    fs::remove_file(fixture.home.join(".ssh")).unwrap();
    fixture.file("project/plain");
    let name = PathBuf::from(OsString::from_vec(vec![0xff]));
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, &name, Access::Read)
            .is_err()
    );
    fixture.file("project/plain");
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("plain"), Access::Read)
            .is_ok()
    );
}

#[test]
fn default_journal_decisions_select_nested_root_and_observe_revocation() {
    use crate::policy::workspace::{CheckedTrust, JournalTrust, WorkspaceTrust as _};
    use maestro_kernel::{
        store::Database,
        workspace::{Answer, Confirmation, WorkspaceAnswer},
    };
    let fixture = Fixture::new();
    let database = Database::open_in(&fixture.home.join("kernel")).unwrap();
    let nested = fixture.project.join("nested");
    fs::create_dir(&nested).unwrap();
    let answer = |path: &Path, answer| {
        database
            .record_workspace_answer(&WorkspaceAnswer {
                path: path.to_path_buf(),
                answer,
            })
            .unwrap();
    };
    for root in [&fixture.project, &nested] {
        answer(
            root,
            Answer::Approved {
                confirmation: Confirmation::ConfirmPath,
            },
        );
    }
    let adapter = JournalTrust::new(&database);
    let checked = CheckedTrust::new(&adapter, &fixture.boundaries);
    assert_eq!(checked.containing_root(&nested), Some(nested.clone()));
    assert!(
        checked
            .authorize(&fixture.project, Path::new("nested/new"), Access::Write)
            .is_ok()
    );
    answer(&nested, Answer::Removed);
    assert_eq!(
        checked.containing_root(&nested),
        Some(fixture.project.clone())
    );
    answer(&fixture.project, Answer::Removed);
    assert!(
        checked
            .authorize(&fixture.project, Path::new("nested/new"), Access::Write)
            .is_err()
    );
    assert!(!nested.join("new").exists());
}

#[test]
fn secret_ancestor_denial_precedes_missing_parent_opens() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.home.join(".ssh")).unwrap();
    fixture.file(".env.base");
    for (base, relative) in [
        (&fixture.home, ".ssh/missing/new"),
        (&fixture.home.join(".ssh"), "missing/new"),
        (&fixture.home.join(".env.base"), "new"),
    ] {
        let error = fixture
            .trust()
            .authorize(base, Path::new(relative), Access::Read)
            .unwrap_err();
        assert_eq!(error.to_string(), "secret or kernel-internal location");
    }
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("new"), Access::Write)
            .is_ok()
    );
}

#[cfg(unix)]
#[test]
fn separator_and_stream_parent_names_refuse_beside_plain_parent() {
    let fixture = Fixture::new();
    for parent in ["plain:stream", "plain\\child", "ordinary"] {
        fixture.file(&format!("project/{parent}/leaf"));
    }
    for path in ["plain:stream/leaf", "plain\\child/leaf"] {
        assert!(
            fixture
                .trust()
                .authorize(&fixture.project, Path::new(path), Access::Read)
                .is_err()
        );
    }
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("ordinary/leaf"), Access::Read)
            .is_ok()
    );
}

#[test]
fn declared_secret_and_plain_aliases_refuse_in_both_directions() {
    let fixture = Fixture::new();
    fixture.file("project/ordinary/plain");
    fixture.file("project/.env.private/plain");
    directory_link(
        &fixture.project.join("ordinary"),
        &fixture.project.join(".env.alias"),
    );
    directory_link(
        &fixture.project.join(".env.private"),
        &fixture.project.join("plain-alias"),
    );
    for base in [
        fixture.project.join(".env.alias"),
        fixture.project.join("plain-alias"),
    ] {
        assert!(
            fixture
                .trust()
                .authorize(&base, Path::new("plain"), Access::Read)
                .is_err()
        );
        assert!(
            fixture
                .trust()
                .authorize(&base, Path::new("new"), Access::Write)
                .is_err()
        );
    }
    assert!(
        fixture
            .trust()
            .authorize(
                &fixture.project.join("ordinary"),
                Path::new("plain"),
                Access::Read
            )
            .is_ok()
    );
    assert!(
        fixture
            .trust()
            .authorize(
                &fixture.project.join("ordinary"),
                Path::new("new"),
                Access::Write
            )
            .is_ok()
    );
    assert!(!fixture.project.join("ordinary/new").exists());
    assert!(!fixture.project.join(".env.private/new").exists());
}

#[test]
fn canonical_ancestry_denies_case_and_normalization_aliases() {
    let mut fixture = Fixture::new();
    fixture.file("project/private-é/kernel/plain");
    fixture.file("project/private-é-notes/kernel/plain");
    fixture.boundaries = TrustBoundaries::with_environment(
        &fixture.home,
        &[fixture.project.join("private-é/kernel")],
        &Environment::default(),
    )
    .unwrap();
    let mut aliases = vec!["private-é", "PRIVATE-é", "PRIVATE-É"];
    if cfg!(target_os = "macos") {
        aliases.push("private-e\u{301}");
    }
    for alias in aliases {
        let named = fixture.project.join(alias);
        if cfg!(windows) || cfg!(target_os = "macos") {
            assert!(
                named.is_dir(),
                "host must exercise the real alias {named:?}"
            );
            let held = Directory::open_canonical(&named).unwrap();
            assert_eq!(
                held.canonical_path().unwrap(),
                fixture.project.join("private-é").canonicalize().unwrap()
            );
        }
        for (leaf, access) in [("plain", Access::Read), ("new", Access::Write)] {
            assert!(
                fixture
                    .trust()
                    .authorize(
                        &fixture.project,
                        &Path::new(alias).join("kernel").join(leaf),
                        access
                    )
                    .is_err(),
                "{alias}/{leaf}"
            );
        }
    }
    for access in [Access::Read, Access::Write] {
        assert!(
            fixture
                .trust()
                .authorize(
                    &fixture.project,
                    Path::new("private-é-notes/kernel/plain"),
                    access
                )
                .is_ok()
        );
    }
    assert!(!fixture.project.join("private-é/kernel/new").exists());
}
