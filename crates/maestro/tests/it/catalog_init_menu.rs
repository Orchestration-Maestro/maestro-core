//! Plain, screen-reader-safe init and the registry-backed config editor.
use super::support::{Home, Running};
use maestro_kernel::scope::LOCAL;
use std::{fs, io::Write as _, path::PathBuf, process::Stdio};

fn catalog() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/catalog/bootstrap/owner-local")
        .canonicalize()
        .unwrap()
}

fn plain(home: &Home, args: &[&str], input: &str) -> super::support::Ended {
    let mut command = home.command(args);
    command
        .current_dir(home.root().join("project"))
        .stdin(Stdio::piped());
    let (running, mut stdin) = Running::with_stdin(command);
    stdin.write_all(input.as_bytes()).unwrap();
    drop(stdin);
    running.finish()
}

#[test]
fn catalog_init_menu_plain_matches_flags_and_preview_writes_nothing() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let catalog = catalog();
    let common = [
        "init",
        "--catalog-dir",
        catalog.to_str().unwrap(),
        "--preset",
        "base",
    ];
    let interactive = plain(
        &home,
        &[&common[..], &["--plain"]].concat(),
        "\n\ny\nfr\nbrief\nupdates=off\n\npreview\n",
    );
    assert_eq!(interactive.code, Some(0), "{interactive:?}");
    let scripted = home.run_in(
        &root,
        &[
            &common[..],
            &[
                "--yes",
                "--language",
                "fr",
                "--tone",
                "brief",
                "--updates",
                "off",
            ],
        ]
        .concat(),
    );
    assert_eq!(scripted.code, Some(0), "{scripted:?}");
    assert_eq!(interactive.stdout, scripted.stdout);
    for label in ["1/5", "2/5", "3/5", "4/5", "5/5"] {
        assert!(interactive.stderr.contains(label), "{interactive:?}");
    }
    assert!(scripted.stderr.is_empty(), "{scripted:?}");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    assert!(!home.data().join("kernel.sqlite3").exists());
}

#[test]
fn catalog_init_menu_cancel_eof_and_interrupt_leave_no_writes() {
    for input in ["cancel\n", "", "\u{3}\n", "\nn\nfr\ncancel\n"] {
        let home = Home::bare();
        fs::create_dir(home.root().join("project")).unwrap();
        let result = plain(&home, &["init", "--plain"], input);
        assert_eq!(result.code, Some(0), "{result:?}");
        assert_eq!(
            fs::read_dir(home.root().join("project")).unwrap().count(),
            0
        );
        assert!(!home.data().join("kernel.sqlite3").exists());
    }
}

#[test]
fn catalog_init_menu_yes_never_grants_trust_and_explicit_setup_applies() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let catalog = catalog();
    let args = [
        "init",
        "--catalog-dir",
        catalog.to_str().unwrap(),
        "--preset",
        "base",
        "--yes",
        "--apply",
    ];
    let refused = home.run_in(&root, &args);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(refused.stderr.contains("maestro trust add"), "{refused:?}");
    assert!(refused.stderr.contains("--confirm-path"), "{refused:?}");
    assert!(!root.join(".maestro").exists());
    assert!(!home.data().join("kernel.sqlite3").exists());
    let approved = home.run(&[
        "trust",
        "add",
        root.to_str().unwrap(),
        "--confirm-path",
        root.to_str().unwrap(),
    ]);
    assert_eq!(approved.code, Some(0), "{approved:?}");
    let applied = home.run_in(&root, &args);
    assert_eq!(applied.code, Some(0), "{applied:?}");
    assert!(root.join(".maestro/config.toml").exists());
}

#[test]
fn catalog_init_menu_config_edits_are_journaled_only_after_confirmation() {
    let home = Home::bare();
    fs::create_dir(home.root().join("project")).unwrap();
    let cancelled = plain(&home, &["config"], "tone=brief\ncancel\n");
    assert_eq!(cancelled.code, Some(0), "{cancelled:?}");
    assert!(!home.config().join("preferences.toml").exists());
    assert!(!home.data().join("kernel.sqlite3").exists());
    let edited = plain(&home, &["config"], "tone=brief\n\nyes\n");
    assert_eq!(edited.code, Some(0), "{edited:?}");
    assert!(edited.stderr.contains("tone ="));
    assert!(edited.stderr.contains("accepts:"));
    assert!(edited.stderr.contains("source:"));
    assert!(
        fs::read_to_string(home.config().join("preferences.toml"))
            .unwrap()
            .contains("brief")
    );
    let history = home.run(&["--json", "config", "history"]);
    assert_eq!(history.code, Some(0), "{history:?}");
    assert!(history.stdout.contains("tone"), "{history:?}");
}

#[test]
fn catalog_init_menu_plain_walkthrough_retains_back_errors_and_fallback_without_color() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let input = format!(
        "{}\nbase\nmaybe\nn\ninvalid-language-tag-extra\nja\nback\nfr\ndetailed\n\
        updates=auto\nupdates=off\n\nback\ntone=brief\n\npreview\n",
        catalog().display()
    );
    let result = plain(&home, &["--no-color", "init", "--plain"], &input);
    assert_eq!(result.code, Some(0), "{result:?}");
    for message in [
        "1/5",
        "2/5",
        "3/5",
        "4/5",
        "5/5",
        "Error:",
        "Interface is English; conversation language remains ja.",
        "user-only",
    ] {
        assert!(result.stderr.contains(message), "{message}: {result:?}");
    }
    assert!(!result.stderr.contains('\u{1b}'));
    assert!(!result.stdout.contains('\u{1b}'));
    assert_eq!(result.stderr.matches("2/5").count(), 2);
    assert_eq!(result.stderr.matches("4/5").count(), 2);
    assert!(result.stdout.contains(".maestro/config.toml"));
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    assert!(!home.data().join("kernel.sqlite3").exists());
}

#[test]
fn catalog_init_menu_plain_staged_trust_never_accepts_piped_authority() {
    for (apply, final_answer, code) in [(false, "preview", 0), (true, "no", 0), (true, "yes", 2)] {
        let home = Home::bare();
        let root = home.root().join("project");
        fs::create_dir(&root).unwrap();
        let catalog = catalog();
        let mut args = vec![
            "init",
            "--plain",
            "--catalog-dir",
            catalog.to_str().unwrap(),
            "--preset",
            "base",
        ];
        if apply {
            args.push("--apply");
        }
        let result = plain(
            &home,
            &args,
            &format!("\n\ny\nen\nnormal\n\n{final_answer}\n"),
        );
        assert_eq!(result.code, Some(code), "{result:?}");
        if code == 2 {
            assert!(result.stderr.contains("maestro trust add"), "{result:?}");
            assert!(result.stderr.contains("--confirm-path"), "{result:?}");
        }
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        assert!(!home.data().join("kernel.sqlite3").exists());
    }
}

#[test]
fn catalog_init_menu_script_and_config_refusals_and_project_layer() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let catalog = catalog();
    let base = [
        "init",
        "--catalog-dir",
        catalog.to_str().unwrap(),
        "--preset",
        "base",
    ];
    for args in [
        &["init", "--yes"][..],
        &["init", "--yes", "--catalog-dir", catalog.to_str().unwrap()],
        &["--json", "config"],
    ] {
        let result = home.run_in(&root, args);
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(result.stdout.is_empty(), "{result:?}");
    }
    for extra in [
        &["--apply"][..],
        &["--yes", "--set", "tone=brief", "--tone", "normal"],
        &["--yes", "--language", "not-valid-long-tag"],
        &["--yes", "--updates", "invalid"],
        &["--yes", "--set", "raw_prompt_logging=true"],
    ] {
        let result = home.run_in(&root, &[&base[..], extra].concat());
        assert_eq!(result.code, Some(2), "{result:?}");
    }
    for input in [
        "raw_prompt_logging=true\ncancel\n",
        "trust=true\ncancel\n",
        "updates=auto\ncancel\n",
        "tone=brief\n\nno\n",
        "tone=brief\n\npreview\n",
    ] {
        let result = plain(&home, &["config", "--project"], input);
        assert_eq!(result.code, Some(0), "{result:?}");
        assert!(!root.join(".maestro/config.toml").exists());
        assert!(!home.data().join("kernel.sqlite3").exists());
    }
    let edited = plain(
        &home,
        &["config", "--project"],
        "tone=brief\n\nback\nupdates=off\n\nyes\n",
    );
    assert_eq!(edited.code, Some(0), "{edited:?}");
    let config = fs::read_to_string(root.join(".maestro/config.toml")).unwrap();
    assert!(config.contains("brief"));
    assert!(config.contains("off"));
    assert!(!home.config().join("preferences.toml").exists());
    assert_eq!(home.database().setting_changes(LOCAL).unwrap().len(), 2);
}

#[test]
fn catalog_init_menu_declined_trust_reviews_only_preferences_and_requires_separate_approval() {
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
    let preview = plain(&home, &args, "\n\nn\nen\nnormal\n\npreview\n");
    assert_eq!(preview.code, Some(0), "{preview:?}");
    assert!(preview.stdout.contains("preferences-only review"));
    assert!(preview.stdout.contains(".maestro/config.toml"));
    assert!(!preview.stdout.contains(".github/copilot-instructions.md"));
    assert!(!preview.stdout.contains(".maestro/project.toml"));
    let refused = plain(
        &home,
        &[&args[..], &["--apply"]].concat(),
        "\n\nn\nen\nnormal\n\nyes\n",
    );
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert!(
        refused.stderr.contains("--preferences-only --confirm-path"),
        "{refused:?}"
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    assert!(!home.data().join("kernel.sqlite3").exists());
}
