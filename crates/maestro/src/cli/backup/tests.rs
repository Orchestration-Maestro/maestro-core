use super::{
    filesystem::{
        create_destination, create_private_dir_all, directory_problem, source_file, timestamp_for,
    },
    names::{ARTIFACTS, DATABASE},
    restore::commit_staged,
    test_support::Scratch,
};
use crate::failure::Failure;
use std::{fs, io, time::Duration};

#[test]
fn destination_checks_missing_empty_and_occupied_paths() {
    let scratch = Scratch::new("destination");
    let missing = scratch.path().join("missing");
    assert_eq!(directory_problem(&missing).unwrap(), None);

    let empty = scratch.path().join("empty");
    fs::create_dir(&empty).unwrap();
    assert_eq!(directory_problem(&empty).unwrap(), None);
    create_destination(&empty).unwrap();

    let occupied = scratch.path().join("occupied");
    fs::create_dir(&occupied).unwrap();
    fs::write(occupied.join("keep"), b"keep").unwrap();
    assert!(directory_problem(&occupied).unwrap().is_some());
}

#[test]
fn destination_rejects_files_and_reports_inspection_errors() {
    let scratch = Scratch::new("destination-errors");
    let file = scratch.path().join("file");
    fs::write(&file, b"not a directory").unwrap();
    assert!(directory_problem(&file).unwrap().is_some());
    assert!(matches!(
        create_destination(&file),
        Err(Failure::Refused(_))
    ));

    let blocked = scratch.path().join("parent-file");
    fs::write(&blocked, b"not a directory").unwrap();
    let child = blocked.join("child");
    assert!(matches!(
        directory_problem(&child),
        Err(Failure::Failed(message)) if message.contains("inspect")
    ));
    assert!(matches!(
        create_destination(&child),
        Err(Failure::Failed(message)) if message.contains("inspect")
    ));
}

#[cfg(unix)]
#[test]
fn destination_rejects_symbolic_link_directories() {
    use std::os::unix::fs::symlink;

    let scratch = Scratch::new("destination-link");
    let directory = scratch.path().join("directory");
    fs::create_dir(&directory).unwrap();
    let link = scratch.path().join("directory-link");
    symlink(&directory, &link).unwrap();
    assert!(directory_problem(&link).unwrap().is_some());
    assert!(matches!(
        create_destination(&link),
        Err(Failure::Refused(_))
    ));
}

#[test]
fn private_directory_creation_rejects_files_and_non_missing_errors() {
    let scratch = Scratch::new("private-directory");
    let nested = scratch.path().join("new").join("nested");
    create_private_dir_all(&nested).unwrap();
    assert!(nested.is_dir());

    let file = scratch.path().join("file");
    fs::write(&file, b"not a directory").unwrap();
    assert!(create_private_dir_all(&file).is_err());
    let error = create_private_dir_all(&file.join("child")).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotADirectory);

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let link = scratch.path().join("directory-link");
        symlink(&nested, &link).unwrap();
        assert!(create_private_dir_all(&link).is_err());
    }
}

#[test]
fn source_file_requires_a_regular_standalone_file() {
    let scratch = Scratch::new("source-file");
    let file = scratch.path().join("source");
    fs::write(&file, b"regular file").unwrap();
    source_file(&file).unwrap();

    let directory = scratch.path().join("directory");
    fs::create_dir(&directory).unwrap();
    assert!(source_file(&directory).is_err());

    #[cfg(unix)]
    {
        use std::os::unix::{fs::symlink, net::UnixListener};

        let link = scratch.path().join("file-link");
        symlink(&file, &link).unwrap();
        assert!(source_file(&link).is_err());

        let socket = scratch.path().join("socket");
        let listener = UnixListener::bind(&socket).unwrap();
        assert!(source_file(&socket).is_err());
        drop(listener);
    }
}

#[test]
fn timestamp_formats_utc_calendar_boundaries_and_milliseconds() {
    let cases = [
        (0, "1970-01-01T00:00:00.000Z"),
        (86_399_999, "1970-01-01T23:59:59.999Z"),
        (951_868_799_999, "2000-02-29T23:59:59.999Z"),
        (951_868_800_000, "2000-03-01T00:00:00.000Z"),
        (2_147_483_647_999, "2038-01-19T03:14:07.999Z"),
        (4_107_542_399_999, "2100-02-28T23:59:59.999Z"),
        (4_107_542_400_000, "2100-03-01T00:00:00.000Z"),
    ];
    for (milliseconds, expected) in cases {
        assert_eq!(timestamp_for(Duration::from_millis(milliseconds)), expected);
    }
}

#[cfg(unix)]
#[test]
fn source_file_rejects_hard_links() {
    let scratch = Scratch::new("source-hardlink");
    let file = scratch.path().join("source");
    fs::write(&file, b"regular file").unwrap();
    let link = scratch.path().join("hard-link");
    fs::hard_link(&file, &link).unwrap();
    assert!(source_file(&file).is_err());
}

#[test]
fn interruption_before_database_commit_leaves_no_kernel_state() {
    let root = Scratch::new("restore-interrupted");
    let data = root.path().join("xdg-data").join("maestro");
    let stage = data.join(".restore-test");
    fs::create_dir_all(stage.join(ARTIFACTS)).unwrap();
    fs::write(stage.join(DATABASE), b"staged database").unwrap();

    let interrupted = commit_staged(&data, &stage, || Err(Failure::failed("interrupted")));
    assert!(interrupted.is_err());
    assert!(!data.join(DATABASE).exists());
    assert!(!data.join(ARTIFACTS).exists());
}
