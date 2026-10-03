use super::{
    PREFIX, RAM, ROOM, VARIABLE, base, create_in, free_space, scratch_directory, scratch_root,
    unpredictable_name,
};
use std::{env, ffi::OsString, fs, io::ErrorKind, path::PathBuf};

/// The platform's temporary directory, as `base` receives it.
fn temp() -> PathBuf {
    PathBuf::from("temp")
}

/// An absolute path on every host, as the variable must name.
fn configured() -> PathBuf {
    env::temp_dir().join("configured")
}

#[test]
fn the_variable_wins_over_the_ram_directory() {
    let chosen = base(Some(configured().into_os_string()), Some(ROOM), temp()).unwrap();
    assert_eq!(chosen, configured());
}

#[test]
fn a_relative_variable_is_refused() {
    let refused = base(Some(OsString::from("relative")), Some(ROOM), temp()).unwrap_err();
    assert_eq!(refused.kind(), ErrorKind::InvalidInput);
    let message = refused.to_string();
    assert!(message.contains(VARIABLE), "{message}");
    assert!(message.contains("must be absolute"), "{message}");
}

#[test]
fn an_empty_variable_counts_as_unset() {
    assert_eq!(
        base(Some(OsString::new()), Some(ROOM), temp()).unwrap(),
        PathBuf::from(RAM)
    );
    assert_eq!(base(Some(OsString::new()), None, temp()).unwrap(), temp());
}

#[test]
fn the_ram_directory_needs_its_full_room() {
    assert_eq!(base(None, Some(ROOM), temp()).unwrap(), PathBuf::from(RAM));
    assert_eq!(base(None, Some(ROOM - 1), temp()).unwrap(), temp());
    assert_eq!(base(None, None, temp()).unwrap(), temp());
}

#[test]
fn the_root_is_chosen_once_and_holds_each_new_directory() {
    let root = scratch_root().unwrap();
    assert!(root.is_absolute(), "{}", root.display());
    assert_eq!(scratch_root().unwrap(), root);
    let directory = scratch_directory().unwrap();
    assert_eq!(directory.parent(), Some(root.as_path()));
    assert!(directory.is_dir(), "{}", directory.display());
    fs::remove_dir(&directory).unwrap();
}

#[test]
fn names_carry_the_prefix_and_differ() {
    let first = unpredictable_name();
    let second = unpredictable_name();
    assert!(first.starts_with(PREFIX), "{first}");
    assert_eq!(first.len(), PREFIX.len() + 16, "{first}");
    assert_ne!(first, second);
}

#[test]
fn a_directory_already_at_a_name_is_never_used() {
    let base = scratch_directory().unwrap();
    fs::create_dir(base.join("taken")).unwrap();
    fs::write(base.join("taken/planted"), b"").unwrap();

    let created = create_in(&base, ["taken".to_owned(), "free".to_owned()]).unwrap();

    assert_eq!(created, base.join("free"));
    assert_eq!(fs::read_dir(base.join("taken")).unwrap().count(), 1);
    fs::remove_dir_all(&base).unwrap();
}

#[test]
fn every_name_taken_is_an_error() {
    let base = scratch_directory().unwrap();
    fs::create_dir(base.join("taken")).unwrap();

    let refused = create_in(&base, ["taken".to_owned()]).unwrap_err();

    assert_eq!(refused.kind(), ErrorKind::AlreadyExists);
    fs::remove_dir_all(&base).unwrap();
}

#[cfg(unix)]
#[test]
fn a_symlink_at_a_name_is_never_followed() {
    use std::os::unix::fs::symlink;

    let base = scratch_directory().unwrap();
    fs::create_dir(base.join("target")).unwrap();
    symlink(base.join("target"), base.join("link")).unwrap();

    let created = create_in(&base, ["link".to_owned(), "free".to_owned()]).unwrap();

    assert_eq!(created, base.join("free"));
    assert!(
        fs::symlink_metadata(base.join("link"))
            .unwrap()
            .is_symlink()
    );
    fs::remove_dir_all(&base).unwrap();
}

#[test]
fn a_disk_scratch_directory_is_new_under_the_temporary_directory() {
    let directory = super::disk_scratch_directory().unwrap();
    assert_eq!(directory.parent(), Some(env::temp_dir().as_path()));
    assert!(directory.is_dir(), "{}", directory.display());
    fs::remove_dir(&directory).unwrap();
}

#[test]
fn a_nonexistent_base_error_is_not_treated_as_a_name_collision() {
    let parent = scratch_directory().unwrap();
    let missing = parent.join("missing");
    let error = create_in(&missing, ["candidate".to_owned()]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NotFound);
    fs::remove_dir(&parent).unwrap();
}

#[cfg(unix)]
#[test]
fn a_new_directory_is_owner_only() {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = scratch_directory().unwrap();
    let mode = fs::metadata(&directory).unwrap().permissions().mode();
    fs::remove_dir(&directory).unwrap();
    assert_eq!(mode & 0o777, 0o700, "{mode:o}");
}

#[cfg(unix)]
#[test]
fn a_directory_reports_its_free_space_and_a_file_none() {
    let directory = scratch_directory().unwrap();
    let free = free_space(&directory).unwrap();
    assert!(free > 1, "{free}");
    let file = directory.join("free-space");
    fs::write(&file, b"").unwrap();
    assert_eq!(free_space(&file), None);
    fs::remove_dir_all(&directory).unwrap();
}

#[cfg(not(unix))]
#[test]
fn a_platform_without_statvfs_reports_no_free_space() {
    let directory = scratch_directory().unwrap();
    assert_eq!(free_space(&directory), None);
    fs::remove_dir(&directory).unwrap();
}

#[cfg(unix)]
#[test]
fn executable_fixtures_keep_their_bytes_mode_and_quoted_path() {
    use super::{disk_scratch_directory, write_executable};
    use std::{os::unix::fs::PermissionsExt as _, process::Command};

    let root = disk_scratch_directory().unwrap();
    let path = root.join("tool with ' quotes $ and spaces");
    let bytes = b"#!/bin/sh\nprintf '%s' 'literal $content'\nexit 7\n";
    write_executable(&path, bytes).unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let output = Command::new(&path).output().unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, b"literal $content");
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn executable_fixtures_refuse_a_failed_writer() {
    use super::{disk_scratch_directory, write_executable};

    let root = disk_scratch_directory().unwrap();
    let error = write_executable(&root.join("absent/tool"), b"").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Other);
    assert!(
        error.to_string().contains("fixture writer exited"),
        "{error}"
    );
    fs::remove_dir_all(root).unwrap();
}
