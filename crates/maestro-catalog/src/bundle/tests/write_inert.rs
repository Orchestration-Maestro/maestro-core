//! Executable-looking assets stay data; filesystem metadata never enters a bundle.

use super::write::{entries, minimal, run};
use crate::{
    files::digest,
    limits::Limits,
    source::{Directory, SourceTree as _, builtin},
};
use maestro_test_scratch::scratch_directory;
use std::{
    fmt::Write as _,
    fs,
    time::{Duration, SystemTime},
};

#[test]
fn templates_hooks_and_scripts_are_inert_and_metadata_is_fixed() {
    let scratch = scratch_directory().unwrap();
    let root = scratch.join("catalog");
    fs::create_dir_all(&root).unwrap();
    let marker = scratch.join("executed");
    let script = format!(
        "#!/bin/sh\nprintf executed > '{}'\n{}",
        marker.display(),
        "# data\n".repeat(2500)
    );
    let mut inventory = "name = 'inert'\nbindings = []\ntools = ['sh']\n".to_owned();
    for name in ["hook.sh", "script.py", "template.sh"] {
        writeln!(
            inventory,
            "[[files]]\nsource = '{name}'\noutput = '{name}'\nsha256 = '{}'",
            digest(script.as_bytes())
        )
        .unwrap();
        let path = root.join(format!("bootstrap/inert/files/{name}"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, &script).unwrap();
    }
    inventory.push_str(
        "[metadata]\nschema = 'maestro-source/2'\nmaturity = 'reviewed'\n\
         rows = ['owner.catalog']\nworkflows = ['ctm-question']\n",
    );
    let package = minimal().read("package.toml", 4096).unwrap();
    fs::write(root.join("package.toml"), package).unwrap();
    fs::write(root.join("bootstrap/inert.toml"), inventory).unwrap();
    let registry = builtin().unwrap();
    let first = run(&Directory::new(&root), &registry, &Limits::PRODUCTION).unwrap();
    for name in ["hook.sh", "script.py", "template.sh"] {
        let path = root.join(format!("bootstrap/inert/files/{name}"));
        let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(123))
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    let second = run(&Directory::new(&root), &registry, &Limits::PRODUCTION).unwrap();
    assert_eq!(first.bytes, second.bytes);
    assert!(!marker.exists());
    let limits = Limits {
        archive_entry_bytes: script.len() as u64,
        ..Limits::PRODUCTION
    };
    assert!(run(&Directory::new(&root), &registry, &limits).is_ok());
    assert!(
        run(
            &Directory::new(&root),
            &registry,
            &Limits {
                archive_entry_bytes: script.len() as u64 - 1,
                ..limits
            }
        )
        .unwrap_err()
        .to_string()
        .contains("archive entry bytes")
    );
    for name in ["hook.sh", "script.py", "template.sh"] {
        assert_eq!(
            entries(&first.bytes)[&format!("bootstrap/inert/files/{name}")],
            script.as_bytes()
        );
    }
    fs::remove_dir_all(scratch).unwrap();
}
