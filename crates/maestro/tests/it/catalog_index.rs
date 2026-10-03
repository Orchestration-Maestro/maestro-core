//! Synthetic public index rendering and exact read-only drift checking.

use super::support::{Ended, Home};
use maestro_catalog::limits::Limits;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// A public-only source checkout, without any release or private mount.
fn catalog(home: &Home) -> PathBuf {
    let root = home.root().join("catalog");
    let fixtures =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/catalog/source");
    for (path, fixture) in [
        ("package.toml", "package.toml"),
        ("core/package.toml", "core-package.toml"),
    ] {
        fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        fs::copy(fixtures.join(fixture), root.join(path)).unwrap();
    }
    root
}

/// Read-only generator; no command depends on an installed kernel.
fn run(home: &Home, root: &Path, view: &str, check: bool) -> Ended {
    let mut args = vec![
        "catalog",
        "index",
        "--catalog-dir",
        root.to_str().unwrap(),
        "--view",
        view,
    ];
    if check {
        args.push("--check");
    }
    home.run(&args)
}

/// Persist the synthetic outputs only in test code.
fn write_views(root: &Path, index: &str, by_type: &str) {
    fs::create_dir_all(root.join("marketplace")).unwrap();
    fs::create_dir_all(root.join("docs/catalog")).unwrap();
    fs::write(root.join("marketplace/index.json"), index).unwrap();
    fs::write(root.join("docs/catalog/by-type.md"), by_type).unwrap();
}

#[test]
fn catalog_index_render_is_deterministic_and_check_is_read_only() {
    let home = Home::bare();
    let root = catalog(&home);
    let first = run(&home, &root, "index", false);
    let second = run(&home, &root, "index", false);
    assert_eq!((first.code, first.stderr.as_str()), (Some(0), ""));
    assert_eq!(first.stdout.as_bytes(), second.stdout.as_bytes());
    let types = run(&home, &root, "by-type", false);
    assert_eq!(types.code, Some(0), "{types:?}");
    assert_eq!(
        types.stdout.as_bytes(),
        run(&home, &root, "by-type", false).stdout.as_bytes()
    );
    assert!(!first.stdout.contains('\r') && !types.stdout.contains('\r'));
    assert!(!root.join("marketplace").exists());
    write_views(&root, &first.stdout, &types.stdout);
    let checked = run(&home, &root, "index", true);
    assert_eq!((checked.code, checked.stderr.as_str()), (Some(0), ""));
    assert_eq!(
        fs::read(root.join("marketplace/index.json")).unwrap(),
        first.stdout.as_bytes()
    );
    assert_eq!(
        fs::read(root.join("docs/catalog/by-type.md")).unwrap(),
        types.stdout.as_bytes()
    );
}

#[test]
fn catalog_index_stale_extra_private_and_missing_refuse() {
    let home = Home::bare();
    let root = catalog(&home);
    let index = run(&home, &root, "index", false);
    let types = run(&home, &root, "by-type", false);
    let missing = run(&home, &root, "index", true);
    assert_eq!(missing.code, Some(2), "{missing:?}");
    assert!(
        missing.stderr.contains("marketplace/index.json") && missing.stderr.contains("missing")
    );
    write_views(&root, &index.stdout, &types.stdout);
    for changed in [
        index.stdout.replace("1.2.3", "1.2.4"),
        format!("{}extra row\n", index.stdout),
        "{\"private\":\"confidential\"}\n".to_owned(),
    ] {
        fs::write(root.join("marketplace/index.json"), &changed).unwrap();
        let refused = run(&home, &root, "index", true);
        assert_eq!(refused.code, Some(2), "{refused:?}");
        assert!(
            refused.stderr.contains("marketplace/index.json")
                && refused.stderr.contains("maestro catalog index")
        );
        assert!(!refused.stderr.contains("confidential"));
        assert_eq!(
            fs::read_to_string(root.join("marketplace/index.json")).unwrap(),
            changed
        );
        assert_eq!(
            run(&home, &root, "index", false).stdout,
            index.stdout,
            "repair must run despite drift"
        );
    }
    fs::write(root.join("marketplace/index.json"), &index.stdout).unwrap();
    fs::write(root.join("docs/catalog/by-type.md"), "stale\n").unwrap();
    let refused = run(&home, &root, "index", true);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(refused.stderr.contains("docs/catalog/by-type.md"));
}

#[test]
fn catalog_index_nonregular_and_oversize_outputs_refuse() {
    let home = Home::bare();
    let root = catalog(&home);
    fs::create_dir_all(root.join("marketplace/index.json")).unwrap();
    let result = run(&home, &root, "index", false);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(result.stderr.contains("regular file"));
    fs::remove_dir(root.join("marketplace/index.json")).unwrap();
    let length = usize::try_from(Limits::PRODUCTION.source_file_bytes).unwrap() + 1;
    fs::write(root.join("marketplace/index.json"), vec![b'x'; length]).unwrap();
    let result = run(&home, &root, "index", true);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(result.stderr.contains("larger than"));
}

#[test]
fn catalog_index_default_and_json_views_preserve_rendered_bytes() {
    let home = Home::bare();
    let root = catalog(&home);
    let args = ["catalog", "index", "--catalog-dir", root.to_str().unwrap()];
    let default = home.run(&args);
    assert_eq!(default.code, Some(0), "{default:?}");
    assert_eq!(default.stdout, run(&home, &root, "index", false).stdout);
    for (view, expected) in [
        ("index", default.stdout),
        ("by-type", run(&home, &root, "by-type", false).stdout),
    ] {
        let json = home.run(&[
            "--json",
            "catalog",
            "index",
            "--catalog-dir",
            root.to_str().unwrap(),
            "--view",
            view,
        ]);
        assert_eq!(json.code, Some(0), "{json:?}");
        let document = json.json();
        assert_eq!(document["schema"], "maestro-cli/catalog-index/1");
        assert_eq!(document["status"], "rendered");
        assert_eq!(document["content"], expected);
    }
    let unreadable = run(&home, &root.join("missing"), "index", false);
    assert_eq!(unreadable.code, Some(1), "{unreadable:?}");
    fs::write(root.join("core/package.toml"), "unknown = true\n").unwrap();
    let invalid = run(&home, &root, "index", false);
    assert_eq!(invalid.code, Some(2), "{invalid:?}");
}

#[cfg(unix)]
#[test]
fn catalog_index_symlink_outputs_refuse_without_disclosing_target() {
    use std::os::unix::fs::symlink;
    let home = Home::bare();
    let root = catalog(&home);
    let target = home.root().join("outside");
    fs::write(&target, "confidential sentinel").unwrap();
    for path in ["marketplace/index.json", "docs/catalog/by-type.md"] {
        fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        symlink(&target, root.join(path)).unwrap();
        for check in [false, true] {
            let result = run(&home, &root, "index", check);
            assert_eq!(result.code, Some(2), "{result:?}");
            assert!(result.stderr.contains(path) && result.stderr.contains("regular file"));
            assert!(!result.stderr.contains("confidential sentinel"));
        }
        fs::remove_file(root.join(path)).unwrap();
    }
    assert_eq!(fs::read_to_string(target).unwrap(), "confidential sentinel");
}
