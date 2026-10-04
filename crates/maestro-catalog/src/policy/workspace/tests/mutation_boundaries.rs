//! Diagnostics and non-UTF-8 ancestry remain observable policy contracts.
#[cfg(unix)]
use crate::policy::workspace::{Access, deny::contains};
use crate::{
    files::tests::support::with_trust,
    policy::workspace::{JournalTrust, deny::Rules},
};
use maestro_test_scratch::scratch_directory;
use std::fs;
#[cfg(unix)]
use std::path::Path;

#[test]
fn trust_debug_output_names_the_wrapper_without_authority_data() {
    let root = scratch_directory().unwrap();
    with_trust(&root, |trust| {
        let text = format!("{trust:?}");
        assert!(
            text.starts_with("CheckedTrust { boundaries: TrustBoundaries"),
            "{text}"
        );
        assert!(text.ends_with(", .. }"), "{text}");
    });
    assert_eq!(
        format!("{:?}", JournalTrust::optional(None)),
        "JournalTrust { .. }"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn deny_patterns_reject_empty_and_dot_basename_aliases() {
    for name in ["", "*", ".", ".."] {
        let text =
            serde_json::json!({"schema": "maestro-secret-paths/1", "paths": [], "names": [name]})
                .to_string();
        assert!(Rules::parse(&text).is_err(), "{name:?}");
    }
}

#[cfg(unix)]
#[test]
fn non_utf8_ancestry_compares_exact_component_bytes() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt as _};
    let root = Path::new("base").join(OsString::from_vec(vec![0xff]));
    assert!(contains(&root, &root.join("child")));
    assert!(!contains(
        &root,
        &Path::new("base")
            .join(OsString::from_vec(vec![0xfe]))
            .join("child")
    ));
}

#[cfg(unix)]
#[test]
fn current_policy_propagates_leaf_canonicalization_errors() {
    use std::os::unix::fs::symlink;
    let root = scratch_directory().unwrap();
    with_trust(&root, |trust| {
        let lease = trust
            .authorize(&root, Path::new("leaf"), Access::Write)
            .unwrap();
        symlink("leaf", root.join("leaf")).unwrap();
        assert!(lease.check_current_policy().is_err());
    });
    fs::remove_dir_all(root).unwrap();
}
