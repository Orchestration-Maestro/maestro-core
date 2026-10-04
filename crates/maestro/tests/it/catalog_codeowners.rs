//! Read-only CODEOWNERS rendering and comparison against committed rules.

use super::support::Home;
use maestro_catalog::limits::Limits;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Two approved synthetic area records, with distinct delegated reviewers.
fn catalog(home: &Home) -> PathBuf {
    let fixtures =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/catalog/source");
    let root = home.root().join("catalog");
    fs::create_dir_all(root.join("core")).unwrap();
    for (path, fixture, owner) in [
        ("package.toml", "package.toml", "root-owner"),
        ("core/package.toml", "core-package.toml", "core-owner"),
    ] {
        let text = fs::read_to_string(fixtures.join(fixture))
            .unwrap()
            .replace("@synthetic/knowledge", owner)
            .replace("description =", "maintainers = [\"reader\"]\ndescription =");
        fs::write(root.join(path), text).unwrap();
    }
    root
}

/// Run only the generator against this root.
fn run(home: &Home, root: &Path, check: bool) -> super::support::Ended {
    let mut args = vec![
        "catalog",
        "codeowners",
        "--catalog-dir",
        root.to_str().unwrap(),
    ];
    if check {
        args.push("--check");
    }
    home.run(&args)
}

#[test]
fn catalog_codeowners_render_and_check_are_read_only() {
    let home = Home::bare();
    let root = catalog(&home);
    let rendered = run(&home, &root, false);
    assert_eq!((rendered.code, rendered.stderr.as_str()), (Some(0), ""));
    assert!(rendered.stdout.contains("/* @root-owner @reader\n"));
    assert!(rendered.stdout.contains("/core/package.toml @core-owner\n"));
    assert!(!root.join(".github").exists(), "render must not write");
    fs::create_dir(root.join(".github")).unwrap();
    fs::write(root.join(".github/CODEOWNERS"), &rendered.stdout).unwrap();
    let checked = run(&home, &root, true);
    assert_eq!((checked.code, checked.stderr.as_str()), (Some(0), ""));
    assert_eq!(
        fs::read_to_string(root.join(".github/CODEOWNERS")).unwrap(),
        rendered.stdout
    );
}

#[test]
fn catalog_codeowners_drift_refuses_without_writing() {
    let home = Home::bare();
    let root = catalog(&home);
    let rendered = run(&home, &root, false);
    assert_eq!(rendered.code, Some(0), "{rendered:?}");
    let missing = run(&home, &root, true);
    assert_eq!(missing.code, Some(2), "{missing:?}");
    assert!(
        missing.stderr.contains("missing") && missing.stderr.contains("/*"),
        "{missing:?}"
    );
    assert!(!root.join(".github").exists());
    fs::create_dir(root.join(".github")).unwrap();
    for (text, rule) in [
        (
            rendered
                .stdout
                .replace("/core/package.toml @core-owner\n", ""),
            "/core/package.toml",
        ),
        (format!("{}/* @reader\n", rendered.stdout), "/*"),
        (
            rendered.stdout.replace(
                "/core/package.toml @core-owner",
                "/core/package.toml @reader",
            ),
            "/core/package.toml",
        ),
    ] {
        fs::write(root.join(".github/CODEOWNERS"), &text).unwrap();
        let result = run(&home, &root, true);
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(result.stderr.contains(rule), "{result:?}");
        assert_eq!(
            fs::read_to_string(root.join(".github/CODEOWNERS")).unwrap(),
            text
        );
    }
}

#[test]
fn catalog_codeowners_nonregular_file_refuses() {
    let home = Home::bare();
    let root = catalog(&home);
    fs::create_dir_all(root.join(".github/CODEOWNERS")).unwrap();
    for result in all_commands(&home, &root) {
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(
            result.stderr.contains("CODEOWNERS") && result.stderr.contains("regular file"),
            "{result:?}"
        );
    }
}

/// Unix can plant a real symlink without Windows' optional symlink privilege.
#[cfg(unix)]
#[test]
fn catalog_codeowners_symlink_refuses() {
    let home = Home::bare();
    let root = catalog(&home);
    fs::create_dir(root.join(".github")).unwrap();
    let target = home.root().join("outside");
    fs::write(&target, "untouched").unwrap();
    symlink(&target, root.join(".github/CODEOWNERS")).unwrap();
    for result in all_commands(&home, &root) {
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(
            result.stderr.contains("CODEOWNERS") && result.stderr.contains("regular file"),
            "{result:?}"
        );
    }
    assert_eq!(fs::read_to_string(target).unwrap(), "untouched");
}

/// The three authoring paths all enforce generated-file safety.
fn all_commands(home: &Home, root: &Path) -> [super::support::Ended; 3] {
    [
        run(home, root, false),
        run(home, root, true),
        home.run(&["catalog", "check", "--catalog-dir", root.to_str().unwrap()]),
    ]
}

#[test]
fn catalog_codeowners_stale_render_succeeds_but_checks_refuse() {
    let home = Home::bare();
    let root = catalog(&home);
    let text = run(&home, &root, false).stdout;
    fs::create_dir(root.join(".github")).unwrap();
    let stale = text.replace(
        "/core/package.toml @core-owner",
        "/core/package.toml @reader",
    );
    fs::write(root.join(".github/CODEOWNERS"), &stale).unwrap();
    let [render, explicit, catalog_check] = all_commands(&home, &root);
    assert_eq!((render.code, render.stdout), (Some(0), text));
    for result in [explicit, catalog_check] {
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(result.stderr.contains("/core/package.toml"), "{result:?}");
    }
    assert_eq!(
        fs::read_to_string(root.join(".github/CODEOWNERS")).unwrap(),
        stale
    );
}

#[test]
fn catalog_codeowners_oversize_refuses_everywhere() {
    let home = Home::bare();
    let root = catalog(&home);
    fs::create_dir(root.join(".github")).unwrap();
    let length = usize::try_from(Limits::PRODUCTION.source_file_bytes).unwrap() + 1;
    fs::write(root.join(".github/CODEOWNERS"), vec![b'x'; length]).unwrap();
    for result in all_commands(&home, &root) {
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(
            result.stderr.contains("CODEOWNERS") && result.stderr.contains("larger than"),
            "{result:?}"
        );
    }
    assert_eq!(
        fs::metadata(root.join(".github/CODEOWNERS")).unwrap().len(),
        u64::try_from(length).unwrap()
    );
}

#[test]
fn catalog_codeowners_stdout_matches_fixture_golden() {
    let home = Home::bare();
    let root = home.root().join("catalog");
    let source = include_str!("../../../../tests/fixtures/catalog/source/core-package.toml");
    for (path, kind, name) in [
        ("package.toml", "package", "common"),
        ("core/package.toml", "package", "core"),
        (
            "capabilities/practice/review/package.toml",
            "package",
            "review",
        ),
        ("languages/rust/package.toml", "language", "rust"),
        ("standards/security/package.toml", "standard", "security"),
    ] {
        let language = if kind == "language" {
            "technology = \"rust\"\n\
             quality_profile = \"quality-profile:rust/default\"\n\
             instructions = [\"instructions:rust/rules\"]\n\
             starter = [\"bootstrap-inventory:rust/starter\"]\n"
        } else {
            ""
        };
        let inventory = if kind == "standard" {
            "rules = [\"SEC-001\"]\n"
        } else {
            ""
        };
        let text = source
            .replace("[metadata]", &format!("{language}{inventory}[metadata]"))
            .replace("kind = \"package\"", &format!("kind = \"{kind}\""))
            .replace("name = \"core\"", &format!("name = \"{name}\""))
            .replace("@synthetic/knowledge", &format!("{name}-owner"))
            .replace(
                "description =",
                &format!("maintainers = [\"reader\", \"@{name}-owner\"]\ndescription ="),
            );
        fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        fs::write(root.join(path), text).unwrap();
    }
    let fixtures =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/catalog/languages");
    for (path, source) in [
        (
            "languages/rust/profiles/quality/default.toml",
            "manager-choices.toml",
        ),
        (
            "languages/rust/instructions/rules.instructions.md",
            "rules.instructions.md",
        ),
        (
            "languages/rust/instructions/rules.maestro.toml",
            "rules.maestro.toml",
        ),
        ("languages/rust/bootstrap/starter.toml", "starter.toml"),
        (
            "standards/quality/profiles/quality/baseline.toml",
            "baseline.toml",
        ),
    ] {
        fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        fs::copy(fixtures.join(source), root.join(path)).unwrap();
    }
    let quality = source
        .replace("name = \"core\"", "name = \"quality\"")
        .replace("kind = \"package\"", "kind = \"standard\"")
        .replace("[metadata]", "rules = [\"quality-001\"]\n[metadata]");
    fs::write(root.join("standards/quality/package.toml"), quality).unwrap();
    let result = run(&home, &root, false);
    assert_eq!(result.code, Some(0), "{result:?}");
    assert_eq!(
        result.stdout,
        include_str!("../../../../tests/fixtures/catalog/codeowners/CODEOWNERS")
    );
    fs::create_dir(root.join(".github")).unwrap();
    fs::write(root.join(".github/CODEOWNERS"), &result.stdout).unwrap();
    for result in all_commands(&home, &root) {
        assert_eq!(result.code, Some(0), "{result:?}");
    }
}

#[test]
fn catalog_codeowners_json_preserves_exact_rendered_bytes() {
    let home = Home::bare();
    let root = catalog(&home);
    let text = run(&home, &root, false).stdout;
    let result = home.run(&[
        "--json",
        "catalog",
        "codeowners",
        "--catalog-dir",
        root.to_str().unwrap(),
    ]);
    assert_eq!(result.code, Some(0), "{result:?}");
    let document = result.json();
    assert_eq!(document["schema"], "maestro-cli/catalog-codeowners/1");
    assert_eq!(document["status"], "rendered");
    assert_eq!(document["codeowners"], text);
}
