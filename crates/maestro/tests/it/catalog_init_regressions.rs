//! C05g review regressions: targeting, parity, retry and the reviewed values.
use super::{
    catalog_init::{approve, copy_tree},
    catalog_init_menu::{catalog, plain},
    support::{Home, make_safe_preferences_path},
};
use maestro_kernel::scope::LOCAL;
use std::fs;

#[test]
fn catalog_init_regression_parent_config_target_refuses_before_effects() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    for target in ["--project", "--user"] {
        for change in [&["set", "updates", "auto"][..], &["unset", "updates"]] {
            let result = home.run_in(&root, &[&["config", target], change].concat());
            assert_eq!(result.code, Some(2), "{result:?}");
            assert!(!home.config().join("preferences.toml").exists());
            assert!(!root.join(".maestro/config.toml").exists());
            assert!(!home.data().join("kernel.sqlite3").exists());
        }
    }
    let project = home.run_in(&root, &["config", "set", "updates", "auto", "--project"]);
    assert_eq!(project.code, Some(2), "{project:?}");
    let user = home.run_in(&root, &["config", "set", "updates", "auto", "--user"]);
    assert_eq!(user.code, Some(0), "{user:?}");
    assert_eq!(home.database().setting_changes(LOCAL).unwrap().len(), 1);
    let before = fs::read(home.config().join("preferences.toml")).unwrap();
    let refused = home.run_in(&root, &["config", "--project", "unset", "updates"]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(
        fs::read(home.config().join("preferences.toml")).unwrap(),
        before
    );
    assert_eq!(home.database().setting_changes(LOCAL).unwrap().len(), 1);
}

#[test]
fn catalog_init_regression_untouched_existing_preferences_preserve_plain_script_parity() {
    for language in ["", "language = 'auto'\n"] {
        let home = Home::bare();
        let root = home.root().join("project");
        fs::create_dir_all(root.join(".maestro")).unwrap();
        let bytes = format!("schema = 'maestro-preferences/1'\n{language}tone = 'brief'\n");
        let config = root.join(".maestro/config.toml");
        fs::write(&config, &bytes).unwrap();
        make_safe_preferences_path(&config);
        make_safe_preferences_path(&root.join(".maestro"));
        let catalog = catalog();
        let base = [
            "init",
            "--catalog-dir",
            catalog.to_str().unwrap(),
            "--preset",
            "base",
        ];
        let scripted = home.run_in(&root, &[&base[..], &["--yes"]].concat());
        let interactive = plain(
            &home,
            &[&base[..], &["--plain"]].concat(),
            "\n\ny\n\n\n\npreview\n",
        );
        assert_eq!(scripted.code, Some(0), "{scripted:?}");
        assert_eq!(interactive.code, Some(0), "{interactive:?}");
        assert_eq!(interactive.stdout, scripted.stdout);
        assert!(!interactive.stderr.contains("Error:"), "{interactive:?}");
        assert_eq!(fs::read(&config).unwrap(), bytes.as_bytes());
        assert!(!home.data().join("kernel.sqlite3").exists());
    }
}

#[test]
fn catalog_init_regression_decline_retry_carries_choices_without_catalog_or_trust() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let result = plain(
        &home,
        &["init", "--plain", "--apply"],
        &format!(
            "{}\nbase\nn\nfr\nbrief\nupdates=off\n\nyes\n",
            catalog().display()
        ),
    );
    assert_eq!(result.code, Some(2), "{result:?}");
    let start = result
        .stderr
        .find("maestro init --apply --preferences-only --confirm-path ")
        .expect("complete retry command");
    let retry = result.stderr[start..].split('`').next().unwrap().trim();
    // The printed POSIX argv uses quoted data; this fixture contains no embedded quotes.
    let args: Vec<_> = retry
        .split_whitespace()
        .skip(1)
        .map(|arg| arg.trim_matches('"'))
        .collect();
    let retried = home.run_in(&root, &args);
    assert_eq!(retried.code, Some(0), "{retry}: {retried:?}");
    let text = fs::read_to_string(root.join(".maestro/config.toml")).unwrap();
    for value in ["language = \"fr\"", "tone = \"brief\"", "updates = \"off\""] {
        assert!(text.contains(value), "{text}");
    }
    assert!(!root.join(".maestro/project.toml").exists());
    assert!(!root.join(".github/copilot-instructions.md").exists());
    assert!(home.database().trusted_workspaces().unwrap().is_empty());
}

#[test]
fn catalog_init_regression_scripted_updates_use_project_validation() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let catalog = catalog();
    let base = [
        "init",
        "--yes",
        "--catalog-dir",
        catalog.to_str().unwrap(),
        "--preset",
        "base",
    ];
    for extra in [&["--updates", "auto"][..], &["--set", "updates=auto"]] {
        let result = home.run_in(&root, &[&base[..], extra].concat());
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(
            result.stderr.contains("updates: auto is user-only"),
            "{result:?}"
        );
    }
    for value in ["off", "propose"] {
        let result = home.run_in(&root, &[&base[..], &["--updates", value]].concat());
        assert_eq!(result.code, Some(0), "{result:?}");
    }
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    assert!(!home.data().join("kernel.sqlite3").exists());
}

#[test]
fn catalog_init_regression_final_review_shows_pinned_toml_and_root() {
    for trusted in [false, true] {
        let home = Home::bare();
        let root = home.root().join("project");
        fs::create_dir(&root).unwrap();
        let catalog = catalog();
        let args = [
            "init",
            "--plain",
            "--catalog-dir",
            catalog.to_str().unwrap(),
            "--preset",
            "base",
        ];
        let result = plain(
            &home,
            &args,
            &format!(
                "\n\n{}\nfr\nbrief\nupdates=off\n\npreview\n",
                if trusted { "y" } else { "n" }
            ),
        );
        assert_eq!(result.code, Some(0), "{result:?}");
        let review = result.stderr.rsplit("5/5").next().unwrap();
        assert!(
            review.contains(&root.canonicalize().unwrap().display().to_string()),
            "{review}"
        );
        assert!(
            review.contains("Config values (.maestro/config.toml):"),
            "{review}"
        );
        let document: serde_json::Value = if trusted {
            serde_json::from_str(&result.stdout).unwrap()
        } else {
            serde_json::from_str(result.stdout.split_once('\n').unwrap().1).unwrap()
        };
        let plan = if trusted {
            &document["preferences"]["files"]
        } else {
            &document["files"]
        };
        let hex = plan["entries"][0]["bytes"].as_str().unwrap();
        let bytes: Vec<_> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let toml = String::from_utf8(bytes).unwrap();
        assert!(review.contains(&toml), "{review}: {toml}");
    }
}

#[test]
fn catalog_init_regression_language_follows_all_settings_and_admitted_defaults() {
    for language in ["fr", "ja"] {
        let home = Home::bare();
        let root = home.root().join("project");
        fs::create_dir(&root).unwrap();
        let catalog_dir = home.root().join("catalog");
        copy_tree(&catalog(), &catalog_dir);
        fs::create_dir(catalog_dir.join("settings")).unwrap();
        fs::write(
            catalog_dir.join("settings/defaults.toml"),
            format!("schema = 'maestro-preferences/1'\nlanguage = '{language}'\n"),
        )
        .unwrap();
        let result = plain(
            &home,
            &[
                "init",
                "--plain",
                "--catalog-dir",
                catalog_dir.to_str().unwrap(),
                "--preset",
                "base",
            ],
            "\n\ny\n\n\n\npreview\n",
        );
        assert_eq!(result.code, Some(0), "{result:?}");
        assert!(
            result.stderr.contains(&format!("language = {language}")),
            "{result:?}"
        );
        assert!(
            result.stderr.contains(if language == "fr" {
                "3/5 Ton"
            } else {
                "Interface is English; conversation language remains ja."
            }),
            "{result:?}"
        );
    }
    let home = Home::bare();
    fs::create_dir(home.root().join("project")).unwrap();
    let result = plain(
        &home,
        &["init", "--plain"],
        &format!(
            "{}\nbase\nn\nen\nnormal\nlanguage=fr\nlanguage=ja\nlanguage=fr\n\npreview\n",
            catalog().display()
        ),
    );
    assert_eq!(result.code, Some(0), "{result:?}");
    assert!(result.stderr.contains("5/5 Vérification"), "{result:?}");
    assert!(
        result
            .stderr
            .contains("Interface is English; conversation language remains ja."),
        "{result:?}"
    );
}

#[test]
fn catalog_init_regression_preferences_write_is_not_reported_as_no_files() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir_all(root.join(".maestro")).unwrap();
    let config = root.join(".maestro/config.toml");
    fs::write(&config, "schema = 'maestro-preferences/1'\n").unwrap();
    make_safe_preferences_path(&config);
    make_safe_preferences_path(&root.join(".maestro"));
    approve(&home, &root);
    let catalog = catalog();
    let args = [
        "init",
        "--yes",
        "--apply",
        "--catalog-dir",
        catalog.to_str().unwrap(),
        "--preset",
        "base",
    ];
    let first = home.run_in(&root, &args);
    assert_eq!(first.code, Some(0), "{first:?}");
    fs::remove_file(&config).unwrap();
    let second = home.run_in(&root, &args);
    assert_eq!(second.code, Some(0), "{second:?}");
    assert!(config.exists());
    assert!(!second.stdout.contains("no files written"), "{second:?}");
}

#[test]
fn catalog_init_regression_explicit_yes_without_apply_on_trusted_root_has_no_effects() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    approve(&home, &root);
    let result = plain(
        &home,
        &["init", "--plain"],
        &format!(
            "{}\nbase\ny\nen\nnormal\n\nyes\npreview\n",
            catalog().display()
        ),
    );
    assert_eq!(result.code, Some(0), "{result:?}");
    assert!(
        result
            .stderr
            .contains("explicit yes when --apply is present"),
        "{result:?}"
    );
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    assert!(home.database().setting_changes(LOCAL).unwrap().is_empty());
}

#[test]
fn catalog_init_regression_yes_and_json_preferences_require_exact_confirmation() {
    for flags in [&["--yes"][..], &["--json"]] {
        let home = Home::bare();
        let root = home.root().join("project");
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let base = [&["init", "--apply", "--preferences-only"][..], flags].concat();
        for confirmation in [None, Some(home.root().to_str().unwrap())] {
            let mut args = base.clone();
            if let Some(path) = confirmation {
                args.extend(["--confirm-path", path]);
            }
            let refused = home.run_in(&root, &args);
            assert_eq!(refused.code, Some(2), "{refused:?}");
            assert!(refused.stderr.contains("--confirm-path"), "{refused:?}");
            assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
            assert!(!home.data().join("kernel.sqlite3").exists());
        }
        let applied = home.run_in(
            &root,
            &[&base[..], &["--confirm-path", root.to_str().unwrap()]].concat(),
        );
        assert_eq!(applied.code, Some(0), "{applied:?}");
        assert!(applied.stderr.is_empty(), "{applied:?}");
        assert!(root.join(".maestro/config.toml").exists());
        assert!(!root.join(".maestro/project.toml").exists());
        assert!(home.database().trusted_workspaces().unwrap().is_empty());
    }
}
