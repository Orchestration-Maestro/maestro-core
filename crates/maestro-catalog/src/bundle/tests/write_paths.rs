//! Ustar limits are archive-format constraints, not new source defaults.

use super::write::{entries, minimal, run};
use crate::{
    files::digest,
    limits::Limits,
    source::{builtin, tests::support::MemoryTree},
};
use std::{collections::BTreeMap, fmt::Write as _, str};

/// A checked exact inventory claiming only the caller's path.
fn asset(path: &str) -> MemoryTree {
    let payload = "inert data";
    let mut inventory = "name = 'paths'\nbindings = []\ntools = []\n".to_owned();
    writeln!(
        inventory,
        "[[files]]\nsource = {path:?}\noutput = 'data.txt'\nsha256 = {:?}",
        digest(payload.as_bytes())
    )
    .unwrap();
    inventory.push_str(
        "[metadata]\nschema = 'maestro-source/2'\nmaturity = 'reviewed'\n\
         rows = ['owner.catalog']\nworkflows = ['ctm-question']\n",
    );
    minimal()
        .with("bootstrap/paths.toml", &inventory)
        .with(&format!("bootstrap/paths/files/{path}"), payload)
}

#[test]
fn descriptor_hook_edges_are_preserved() {
    let tree = asset("data.txt").with(
        "presets/inert.toml",
        "name = 'inert'\ndescription = 'Synthetic preset'\ntemplates = ['common/paths']\n\
         [metadata]\nschema = 'maestro-source/2'\nmaturity = 'reviewed'\n\
         rows = ['owner.catalog']\nworkflows = ['ctm-question']\nrequires = []\n",
    );
    let bundle = run(&tree, &builtin().unwrap(), &Limits::PRODUCTION).unwrap();
    assert_eq!(
        bundle.manifest.closures["preset:inert"],
        [
            "bootstrap-inventory:common/paths",
            "package:common",
            "preset:inert"
        ]
    );
    assert_eq!(
        bundle.manifest.resources["preset:inert"].requires,
        ["bootstrap-inventory:common/paths", "package:common"]
    );
    assert_eq!(
        bundle.manifest.entry_points["package:common"],
        Vec::<String>::new()
    );
}

#[test]
fn all_descriptor_area_runtime_constraints_are_preserved() {
    let runtime = format!("={}", env!("CARGO_PKG_VERSION"));
    let standard = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/catalog/bootstrap/owner-local/",
        "standards/security/package.toml"
    ));
    let tree = minimal().with(
        "standards/security/package.toml",
        &format!("runtime = {runtime:?}\n{standard}"),
    );
    let bundle = run(&tree, &builtin().unwrap(), &Limits::PRODUCTION).unwrap();
    assert_eq!(
        bundle.manifest.requires.runtime,
        BTreeMap::from([("standard:security".to_owned(), runtime)])
    );
    assert_eq!(
        bundle.manifest.entry_points["standard:security"],
        Vec::<String>::new()
    );
    assert!(
        !bundle
            .manifest
            .requires
            .runtime
            .contains_key("package:common")
    );
}

#[test]
fn ustar_name_accepts_100_bytes_and_refuses_101() {
    let registry = builtin().unwrap();
    let name = "a".repeat(100);
    let bundle = run(&asset(&name), &registry, &Limits::PRODUCTION).unwrap();
    assert_eq!(
        entries(&bundle.bytes)[&format!("bootstrap/paths/files/{name}")],
        b"inert data"
    );
    let error = run(&asset(&"a".repeat(101)), &registry, &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(error.contains("path cannot fit ustar"), "{error}");
}

#[test]
fn ustar_prefix_split_accepts_maximum_and_refuses_one_past() {
    let registry = builtin().unwrap();
    let prefix = "bootstrap/paths/files/";
    let directory = "d".repeat(155 - prefix.len());
    let name = "n".repeat(100);
    let path = format!("{directory}/{name}");
    let bundle = run(&asset(&path), &registry, &Limits::PRODUCTION).unwrap();
    assert_eq!(
        entries(&bundle.bytes)[&format!("{prefix}{path}")],
        b"inert data"
    );
    // Inspect the split itself rather than accepting a GNU/PAX workaround.
    let expected = format!("{prefix}{directory}");
    assert!(
        bundle
            .bytes
            .as_chunks::<512>()
            .0
            .iter()
            .any(|block| str::from_utf8(&block[345..500]).is_ok_and(|text| text == expected))
    );
    let past = format!("{directory}d/{name}");
    let error = run(&asset(&past), &registry, &Limits::PRODUCTION)
        .unwrap_err()
        .to_string();
    assert!(error.contains("path cannot fit ustar"), "{error}");
}
