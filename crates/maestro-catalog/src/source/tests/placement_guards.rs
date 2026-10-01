//! Isolated registration and filename classification guards.

use super::area_support::{folder, scoped};
use crate::source::{Layout, MetadataPlace, Registry};

#[test]
fn registry_refuses_partial_wildcard_segments() {
    let mut descriptor = scoped("glossaries", &["core"]);
    descriptor.layout = Layout::Files {
        suffix: ".toml".to_owned(),
        folders: vec!["rev*".to_owned()],
    };
    assert!(Registry::default().register(descriptor).is_err());
}

#[test]
fn registry_refuses_empty_folder_asset() {
    assert!(Registry::default().register(folder(&[""])).is_err());
}

#[test]
fn registry_refuses_nested_folder_primary() {
    let mut descriptor = folder(&[]);
    descriptor.layout = Layout::Folder {
        file: "nested/SKILL.md".to_owned(),
        data: vec![],
    };
    assert!(Registry::default().register(descriptor).is_err());
}

#[test]
fn registry_sidecar_suffix_is_portable_and_nonempty() {
    for suffix in [
        "",
        "/metadata.toml",
        "\\metadata.toml",
        ":metadata.toml",
        "*.toml",
    ] {
        let mut descriptor = scoped("glossaries", &["core"]);
        descriptor.metadata = MetadataPlace::Sidecar {
            suffix: suffix.to_owned(),
        };
        assert!(
            Registry::default().register(descriptor).is_err(),
            "{suffix:?}"
        );
    }
    let mut descriptor = scoped("glossaries", &["core"]);
    descriptor.metadata = MetadataPlace::Sidecar {
        suffix: ".maestro.toml".to_owned(),
    };
    assert!(Registry::default().register(descriptor).is_ok());
}

// Unix allows byte names; Windows exposes Unicode names through its directory API.
#[cfg(unix)]
#[test]
fn directory_non_utf8_names_are_unsupported() {
    use crate::source::{Directory, EntryKind, SourceTree};
    use std::{ffi::OsString, fs, os::unix::ffi::OsStringExt};
    let scratch = maestro_test_scratch::scratch_directory().unwrap();
    fs::write(scratch.join(OsString::from_vec(vec![0x80])), b"").unwrap();
    let entries = Directory::new(&scratch).list("").unwrap();
    fs::remove_dir_all(scratch).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].kind, EntryKind::Unsupported);
}
