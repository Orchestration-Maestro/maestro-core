//! Installed metadata and short/changed-length read neighbours.
use super::{
    launch::{bootstrap, installed, read_sized, trusted_metadata},
    port::Refusal,
    test_support::{directory, pin},
};
use rustix::io::{FdFlags, fcntl_getfd};
use std::{
    fs::{self, File},
    io::Cursor,
    path::Path,
};
#[test]
fn n17_metadata_lengths_and_installed_bootstrap_positive_and_negative() {
    let parent = directory();
    let path = Path::new("/bin/true");
    installed(&File::open(path).unwrap()).unwrap();
    let mut remaining = fs::metadata(path).unwrap().len();
    let file = bootstrap(pin(path), true, &mut remaining).unwrap();
    assert_eq!(remaining, 0);
    assert_eq!(fcntl_getfd(&file).unwrap(), FdFlags::empty());
    for (regular, uid, mode, executable) in [
        (false, 0, 0o555, true),
        (true, 1, 0o555, true),
        (true, 0, 0o575, false),
        (true, 0, 0o557, false),
        (true, 0, 0o444, true),
    ] {
        assert!(!trusted_metadata(regular, uid, mode, executable));
    }
    assert!(trusted_metadata(true, 0, 0o444, false));
    assert!(trusted_metadata(true, 0, 0o500, true));
    assert_eq!(
        read_sized(&mut Cursor::new(b"short"), 6),
        Err(Refusal::LaunchPin)
    );
    assert_eq!(
        read_sized(&mut Cursor::new(b"grown"), 4),
        Err(Refusal::LaunchPin)
    );
    assert_eq!(
        read_sized(&mut Cursor::new(b"exact"), 5),
        Ok(b"exact".to_vec())
    );
    fs::remove_dir_all(parent).unwrap();
}
