//! Effect-time secret rebinding and checked-parent creation leases.
use super::{
    Access, CheckedTrust, Environment, Fixture, Path, TrustBoundaries, directory_link, fs,
};

#[test]
fn review_secret_binding_swap_rechecks_retained_and_new_decisions() {
    let mut fixture = Fixture::new();
    fixture.file("project/keys/key");
    fixture.file("project/notes/key");
    fixture.file("project/neighbour/plain");
    directory_link(&fixture.project.join("keys"), &fixture.home.join(".ssh"));
    fixture.boundaries =
        TrustBoundaries::with_environment(&fixture.home, &[], &Environment::default()).unwrap();
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("keys/key"), Access::Read)
            .is_err()
    );
    let retained = fixture
        .trust()
        .authorize(&fixture.project, Path::new("notes/key"), Access::Read)
        .unwrap();
    #[cfg(unix)]
    fs::remove_file(fixture.home.join(".ssh")).unwrap();
    #[cfg(windows)]
    fs::remove_dir(fixture.home.join(".ssh")).unwrap();
    directory_link(&fixture.project.join("notes"), &fixture.home.join(".ssh"));
    let fresh =
        TrustBoundaries::with_environment(&fixture.home, &[], &Environment::default()).unwrap();
    assert!(
        CheckedTrust::new(&fixture.roots, &fresh)
            .authorize(&fixture.project, Path::new("notes/key"), Access::Read)
            .is_err()
    );
    let retained_read_ok = retained.open_read().is_ok();
    let stale_write_ok = fixture
        .trust()
        .authorize(&fixture.project, Path::new("notes/new"), Access::Write)
        .and_then(|grant| grant.create_new())
        .is_ok();
    let secret_new_exists = fixture.project.join("notes/new").exists();
    assert!(!retained_read_ok && !stale_write_ok && !secret_new_exists);
    assert!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("neighbour/plain"), Access::Read)
            .unwrap()
            .open_read()
            .is_ok()
    );
    fixture
        .trust()
        .authorize(&fixture.project, Path::new("neighbour/new"), Access::Write)
        .unwrap()
        .create_new()
        .unwrap();
}

#[test]
fn parent_move_between_check_and_create_is_refused_and_rolled_back() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.project.join("parent")).unwrap();
    let grant = fixture
        .trust()
        .authorize(&fixture.project, Path::new("parent/new"), Access::Write)
        .unwrap();
    let moved = fixture.outside.join("parent");
    let result = grant.create_new_with(
        || {
            #[cfg(unix)]
            {
                fs::rename(fixture.project.join("parent"), &moved).unwrap();
                directory_link(&fixture.outside, &fixture.project.join("parent"));
            }
            #[cfg(windows)]
            assert!(fs::rename(fixture.project.join("parent"), &moved).is_err());
        },
        || {},
    );
    #[cfg(unix)]
    assert!(result.is_err());
    #[cfg(windows)]
    {
        drop(result.unwrap());
        assert!(fixture.project.join("parent/new").exists());
    }
    assert!(!moved.join("new").exists());
    drop(grant);
    fixture
        .trust()
        .authorize(&fixture.project, Path::new("neighbour"), Access::Write)
        .unwrap()
        .create_new()
        .unwrap();
    assert!(fixture.project.join("neighbour").exists());
}

#[test]
fn outside_base_writes_only_a_trusted_descendant_not_an_outside_target() {
    let fixture = Fixture::new();
    fixture
        .trust()
        .authorize(&fixture.home, Path::new("project/new"), Access::Write)
        .unwrap()
        .create_new()
        .unwrap();
    assert!(fixture.project.join("new").exists());
    assert!(
        fixture
            .trust()
            .authorize(&fixture.home, Path::new("project-other/new"), Access::Write)
            .is_err()
    );
    assert!(!fixture.outside.join("new").exists());
}

#[cfg(unix)]
#[test]
fn rollback_preserves_a_replacement_and_reports_refusal() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.project.join("parent")).unwrap();
    let grant = fixture
        .trust()
        .authorize(&fixture.project, Path::new("parent/new"), Access::Write)
        .unwrap();
    let moved = fixture.outside.join("parent");
    let result = grant.create_new_with(
        || {
            fs::rename(fixture.project.join("parent"), &moved).unwrap();
            directory_link(&fixture.outside, &fixture.project.join("parent"));
        },
        || {
            fs::remove_file(moved.join("new")).unwrap();
            fs::write(moved.join("new"), b"").unwrap();
        },
    );
    assert!(
        moved.join("new").exists(),
        "replacement must survive rollback"
    );
    assert_eq!(fs::read(moved.join("new")).unwrap(), b"");
    let error = result.unwrap_err().to_string();
    assert!(error.contains("parent changed"));
    assert!(error.contains("rollback failed"));
    assert!(error.contains("replacement not deleted"));
    drop(grant);
    fixture
        .trust()
        .authorize(&fixture.project, Path::new("neighbour"), Access::Write)
        .unwrap()
        .create_new()
        .unwrap();
}

#[test]
fn dangling_secret_binding_refuses_new_target_beside_a_healthy_binding() {
    let fixture = Fixture::new();
    let grant = fixture
        .trust()
        .authorize(&fixture.project, Path::new("new"), Access::Write)
        .unwrap();
    let link = fixture.home.join(".netrc");
    #[cfg(unix)]
    super::symlink(fixture.project.join("new"), &link).unwrap();
    #[cfg(windows)]
    {
        let output = super::Command::new("cmd")
            .args(["/C", "mklink"])
            .arg(&link)
            .arg(fixture.project.join("new"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "host must permit this file-symlink proof: {output:?}"
        );
    }
    assert!(
        TrustBoundaries::with_environment(&fixture.home, &[], &Environment::default()).is_err()
    );
    assert!(grant.create_new().is_err());
    assert!(!fixture.project.join("new").exists());
    drop(grant);
    fs::remove_file(link).unwrap();
    fixture
        .trust()
        .authorize(&fixture.project, Path::new("neighbour"), Access::Write)
        .unwrap()
        .create_new()
        .unwrap();
    assert!(fixture.project.join("neighbour").exists());
}

#[test]
fn canonicalization_error_refuses_even_if_location_is_now_absent() {
    use crate::policy::workspace::deny::canonical_location_failure;
    let fixture = Fixture::new();
    assert!(canonical_location_failure(&fixture.project.join("absent")).is_err());
    fixture
        .trust()
        .authorize(&fixture.project, Path::new("neighbour"), Access::Write)
        .unwrap()
        .create_new()
        .unwrap();
}

#[test]
fn deny_rebind_between_check_and_create_rolls_back() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.project.join("notes")).unwrap();
    let grant = fixture
        .trust()
        .authorize(&fixture.project, Path::new("notes/new"), Access::Write)
        .unwrap();
    let result = grant.create_new_with(
        || directory_link(&fixture.project.join("notes"), &fixture.home.join(".ssh")),
        || {},
    );
    assert!(result.is_err());
    assert!(!fixture.project.join("notes/new").exists());
    let fresh =
        TrustBoundaries::with_environment(&fixture.home, &[], &Environment::default()).unwrap();
    assert!(
        CheckedTrust::new(&fixture.roots, &fresh)
            .authorize(&fixture.project, Path::new("notes/new"), Access::Write)
            .is_err()
    );
    drop(grant);
    #[cfg(unix)]
    fs::remove_file(fixture.home.join(".ssh")).unwrap();
    #[cfg(windows)]
    fs::remove_dir(fixture.home.join(".ssh")).unwrap();
    fixture
        .trust()
        .authorize(&fixture.project, Path::new("neighbour"), Access::Write)
        .unwrap()
        .create_new()
        .unwrap();
    assert!(fixture.project.join("neighbour").exists());
}

#[test]
fn deny_rebind_between_check_and_read_drops_handle() {
    let fixture = Fixture::new();
    fixture.file("project/notes/plain");
    fixture.file("project/neighbour/plain");
    let grant = fixture
        .trust()
        .authorize(&fixture.project, Path::new("notes/plain"), Access::Read)
        .unwrap();
    let result = grant.open_read_with(|| {
        directory_link(&fixture.project.join("notes"), &fixture.home.join(".ssh"));
    });
    assert!(result.is_err());
    assert_eq!(
        fixture
            .trust()
            .authorize(&fixture.project, Path::new("neighbour/plain"), Access::Read)
            .unwrap()
            .open_read()
            .unwrap()
            .metadata()
            .unwrap()
            .len(),
        9
    );
}

#[cfg(unix)]
#[test]
fn rollback_survives_owner_unreadable_created_file() {
    use std::{os::unix::fs::PermissionsExt as _, process::Command};
    let user = Command::new("id").arg("-u").output().unwrap();
    assert!(user.status.success());
    if user.stdout == b"0\n" {
        return;
    }
    let fixture = Fixture::new();
    fs::create_dir(fixture.project.join("parent")).unwrap();
    let grant = fixture
        .trust()
        .authorize(&fixture.project, Path::new("parent/new"), Access::Write)
        .unwrap();
    let moved = fixture.outside.join("parent");
    let result = grant.create_new_with(
        || {
            fs::rename(fixture.project.join("parent"), &moved).unwrap();
            directory_link(&fixture.outside, &fixture.project.join("parent"));
        },
        || fs::set_permissions(moved.join("new"), fs::Permissions::from_mode(0o200)).unwrap(),
    );
    assert!(result.is_err());
    assert!(!moved.join("new").exists());
    assert!(fs::read_dir(&moved).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".new.maestro-quarantine-")
    }));
    drop(grant);
    fixture
        .trust()
        .authorize(&fixture.project, Path::new("neighbour"), Access::Write)
        .unwrap()
        .create_new()
        .unwrap();
    assert!(fixture.project.join("neighbour").exists());
}
