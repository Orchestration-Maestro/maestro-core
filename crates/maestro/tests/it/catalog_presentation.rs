//! C05c keeps localized interface prose separate from English machine contracts.
use super::support::{Ended, Home, Running, initialize_mcp, make_safe_preferences_path};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// All three conversational tones must select the same interface template.
pub(super) const TONES: [&str; 3] = ["brief", "normal", "detailed"];

/// An isolated project, never granted trust by a language preference.
pub(super) fn project(home: &Home) -> PathBuf {
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    root.canonicalize().unwrap()
}

/// Run without color and without usable model/network endpoints.
fn plain(home: &Home, root: &Path, args: &[&str]) -> Ended {
    let mut command = home.command(args);
    command.current_dir(root).env("NO_COLOR", "1");
    command.env("MAESTRO_ROUTER_URL", "http://127.0.0.1:0");
    command.env("MAESTRO_QDRANT_URL", "http://127.0.0.1:0");
    if args.contains(&"init") {
        if !args.contains(&"--yes") {
            command.arg("--yes");
        }
        let catalog = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/catalog/bootstrap/owner-local");
        command.args([
            "--catalog-dir",
            catalog.to_str().unwrap(),
            "--preset",
            "base",
        ]);
        if !args.contains(&"--apply") {
            command.arg("--apply");
        }
    }
    Running::of(command).finish()
}

#[test]
fn catalog_presentation_init_trust_is_localized_and_tone_invariant() {
    let home = Home::bare();
    let root = project(&home);
    for (language, expected) in [
        ("en", "workspace is untrusted;"),
        ("fr", "l’espace de travail n’est pas approuvé ;"),
        ("es", "el espacio de trabajo no es de confianza;"),
        ("ja", "workspace is untrusted;"),
    ] {
        let mut baseline = None;
        for tone in TONES {
            let result = plain(
                &home,
                &root,
                &["--language", language, "--tone", tone, "init", "--apply"],
            );
            assert_eq!(result.code, Some(2), "{result:?}");
            assert!(result.stderr.contains(expected), "{result:?}");
            assert!(result.stderr.contains("maestro trust add"), "{result:?}");
            assert!(result.stderr.contains("--confirm-path"), "{result:?}");
            assert!(!result.stderr.contains('\u{1b}'));
            if let Some(before) = &baseline {
                assert_eq!(&result.stderr, before, "tone changed interface prose");
            }
            baseline = Some(result.stderr);
        }
    }
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
}

#[test]
fn catalog_presentation_fallback_note_is_once_only_for_human_cli() {
    let home = Home::bare();
    for language in ["en", "fr", "es", "ja", "zh-Hant-TW", "auto"] {
        for tone in TONES {
            let result = home.run(&[
                "--language",
                language,
                "--tone",
                tone,
                "config",
                "get",
                "models.compute",
            ]);
            assert_eq!(result.code, Some(0), "{result:?}");
            let fallback = !["en", "fr", "es", "auto"].contains(&language);
            let note = format!("Interface is English; conversation language remains {language}.\n");
            assert_eq!(result.stdout, "gpu\n");
            assert_eq!(result.stderr, if fallback { note } else { String::new() });
            let json = home.run(&[
                "--json",
                "--language",
                language,
                "--tone",
                tone,
                "config",
                "get",
                "models.compute",
            ]);
            assert_eq!(json.code, Some(0), "{json:?}");
            assert_eq!(json.stderr, "");
            assert_eq!(json.json()["value"], "gpu");
            let (child, mut input) =
                home.start_with_stdin(&["--language", language, "--tone", tone, "mcp"]);
            initialize_mcp(&mut input);
            drop(input);
            let mcp = child.finish();
            assert_eq!(mcp.code, Some(0), "{mcp:?}");
            assert!(!mcp.stdout.contains("Interface is English"));
            assert!(!mcp.stderr.contains("Interface is English"));
            let _document: serde_json::Value = serde_json::from_str(mcp.stdout.trim()).unwrap();
        }
    }
}

#[test]
fn catalog_presentation_confirmation_and_outcome_use_the_message_port() {
    for (language, confirmation, outcome) in [
        (
            "en",
            "preferences-only write requires separate confirmation:",
            "Wrote preferences only; no workspace trust granted.\n",
        ),
        (
            "fr",
            "l’écriture des préférences seules nécessite une confirmation distincte :",
            "Préférences seules écrites ; aucune confiance accordée à l’espace de travail.\n",
        ),
        (
            "es",
            "escribir solo las preferencias requiere una confirmación independiente:",
            "Solo se escribieron las preferencias; no se otorgó confianza al espacio de trabajo.\n",
        ),
        (
            "ja",
            "preferences-only write requires separate confirmation:",
            "Wrote preferences only; no workspace trust granted.\n",
        ),
    ] {
        for tone in TONES {
            let home = Home::bare();
            let root = project(&home);
            let args = [
                "--language",
                language,
                "--tone",
                tone,
                "init",
                "--yes",
                "--preferences-only",
            ];
            let refused = plain(&home, &root, &args);
            assert_eq!(refused.code, Some(2), "{refused:?}");
            assert!(refused.stderr.contains(confirmation), "{refused:?}");
            assert!(
                refused.stderr.contains("--preferences-only --confirm-path"),
                "{refused:?}"
            );
            assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
            let args = [&args[..], &["--confirm-path", root.to_str().unwrap()]].concat();
            let written = plain(&home, &root, &args);
            assert_eq!(written.code, Some(0), "{written:?}");
            assert!(written.stdout.ends_with(outcome), "{written:?}");
            assert_eq!(
                written.stderr,
                if language == "ja" {
                    "Interface is English; conversation language remains ja.\n"
                } else {
                    ""
                }
            );
            assert!(home.database().trusted_workspaces().unwrap().is_empty());
        }
    }
}

#[test]
fn catalog_presentation_json_and_english_diagnostics_are_byte_invariant() {
    let home = Home::bare();
    let root = project(&home);
    let mut baseline = None;
    let catalog = super::catalog_check::valid_catalog(&home);
    for language in ["en", "fr", "es", "ja"] {
        for tone in TONES {
            let result = home.run_in(
                &root,
                &[
                    "--json",
                    "--language",
                    language,
                    "--tone",
                    tone,
                    "config",
                    "get",
                    "models.compute",
                ],
            );
            assert_eq!(result.code, Some(0), "{result:?}");
            let refused = plain(
                &home,
                &root,
                &[
                    "--json",
                    "--language",
                    language,
                    "--tone",
                    tone,
                    "init",
                    "--yes",
                    "--apply",
                ],
            );
            assert_eq!(refused.code, Some(2), "{refused:?}");
            assert_eq!(refused.stdout, "");
            assert!(refused.stderr.contains("workspace is untrusted;"));
            let checked = home.run(&[
                "--json",
                "--language",
                language,
                "--tone",
                tone,
                "catalog",
                "check",
                "--catalog-dir",
                catalog.to_str().unwrap(),
            ]);
            assert_eq!(checked.code, Some(0), "{checked:?}");
            assert_eq!(checked.json()["status"], "passed");
            assert_eq!(checked.json()["schema"], "maestro-cli/catalog-check/2");
            assert_eq!(checked.stderr, "");
            let pair = (result.stdout, refused.stderr, checked.stdout);
            if let Some(before) = &baseline {
                assert_eq!(&pair, before);
            }
            baseline = Some(pair);
        }
    }
}

#[test]
fn catalog_presentation_help_and_complete_warnings_remain_english() {
    let home = Home::bare();
    let root = project(&home);
    fs::create_dir(root.join(".maestro")).unwrap();
    fs::write(
        root.join(".maestro/config.toml"),
        "invalid planted preferences",
    )
    .unwrap();
    // Plant a real other-writable candidate, so startup warns without parsing it.
    make_safe_preferences_path(&root);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(root.join(".maestro"), fs::Permissions::from_mode(0o777)).unwrap();
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        assert!(
            Command::new("icacls")
                .arg(root.join(".maestro"))
                .args(["/grant", "*S-1-1-0:(W)"])
                .status()
                .unwrap()
                .success()
        );
    }
    let baseline = home.run(&["--help"]);
    let mut warning = None;
    for language in ["en", "fr", "es", "ja"] {
        for tone in TONES {
            let help = home.run(&["--language", language, "--tone", tone, "--help"]);
            assert_eq!(help.stdout, baseline.stdout);
            assert_eq!(help.stderr, baseline.stderr);
            let result = home.run_in(
                &root,
                &[
                    "--json",
                    "--language",
                    language,
                    "--tone",
                    tone,
                    "config",
                    "get",
                    "models.compute",
                ],
            );
            assert_eq!(result.code, Some(0), "{result:?}");
            assert!(result.stderr.contains("skipped"), "{result:?}");
            if let Some(before) = &warning {
                assert_eq!(&result.stderr, before);
            }
            warning = Some(result.stderr);
        }
    }
}

#[test]
fn catalog_presentation_review_resolution_precedes_fallback() {
    let home = Home::bare();
    let root = project(&home);
    fs::create_dir(root.join(".maestro")).unwrap();
    let file = root.join(".maestro/config.toml");
    let valid = "schema = 'maestro-preferences/1'\nlanguage = 'fr'\n";
    fs::write(&file, valid).unwrap();
    make_safe_preferences_path(&root.join(".maestro"));
    make_safe_preferences_path(&file);
    for tone in TONES {
        let selected = home.run_in(
            &root,
            &[
                "--language",
                "ja",
                "--tone",
                tone,
                "config",
                "get",
                "language",
            ],
        );
        assert_eq!(selected.code, Some(0), "{selected:?}");
        assert_eq!(selected.stdout, "ja\n");
        assert_eq!(
            selected.stderr,
            "Interface is English; conversation language remains ja.\n"
        );
        let json = home.run_in(
            &root,
            &[
                "--json",
                "--language",
                "ja",
                "--tone",
                tone,
                "config",
                "get",
                "language",
            ],
        );
        assert_eq!(json.code, Some(0), "{json:?}");
        assert_eq!(json.json()["value"], "ja");
        let neighbour = home.run_in(&root, &["--tone", tone, "config", "get", "language"]);
        assert_eq!(neighbour.code, Some(0), "{neighbour:?}");
        assert_eq!(neighbour.stdout, "fr\n");
        assert_eq!(fs::read_to_string(&file).unwrap(), valid);
    }
    fs::write(&file, "schema = 'maestro-preferences/1'\nunknown = true\n").unwrap();
    for tone in TONES {
        let refused = home.run_in(
            &root,
            &[
                "--language",
                "ja",
                "--tone",
                tone,
                "config",
                "get",
                "language",
            ],
        );
        assert_eq!(refused.code, Some(2), "{refused:?}");
        assert!(refused.stderr.contains("unknown"), "{refused:?}");
        assert_eq!(
            refused.stdout, "",
            "fallback precedes session refusal: {refused:?}"
        );
        assert!(
            !refused.stderr.contains("Interface is English"),
            "{refused:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn catalog_presentation_review_unsafe_path_instructions() {
    let home = Home::bare();
    let root = home.root().join("équipe {$literal}");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let escaped = format!("{:?}", root.to_str().unwrap());
    for (language, trust_data, confirm_data) in [
        (
            "en",
            "canonical path (data):",
            "quote the canonical path for your shell; path (data):",
        ),
        (
            "fr",
            "chemin canonique (donnée) :",
            concat!(
                "mettez le chemin canonique entre guillemets, ",
                "avec les échappements adaptés à votre shell ; chemin (donnée) :"
            ),
        ),
        (
            "es",
            "ruta canónica (dato):",
            "entrecomille la ruta canónica para su shell; ruta (dato):",
        ),
        (
            "ja",
            "canonical path (data):",
            "quote the canonical path for your shell; path (data):",
        ),
    ] {
        for tone in TONES {
            let untrusted = plain(
                &home,
                &root,
                &["--language", language, "--tone", tone, "init", "--apply"],
            );
            assert_eq!(untrusted.code, Some(2), "{untrusted:?}");
            assert!(untrusted.stderr.contains(trust_data), "{untrusted:?}");
            assert!(untrusted.stderr.contains(&escaped), "{untrusted:?}");
            assert!(
                !untrusted.stderr.contains("maestro trust add \""),
                "{untrusted:?}"
            );
            let confirmation = plain(
                &home,
                &root,
                &[
                    "--language",
                    language,
                    "--tone",
                    tone,
                    "init",
                    "--yes",
                    "--preferences-only",
                ],
            );
            assert_eq!(confirmation.code, Some(2), "{confirmation:?}");
            assert!(
                confirmation.stderr.contains(confirm_data),
                "{confirmation:?}"
            );
            assert!(confirmation.stderr.contains(&escaped), "{confirmation:?}");
            assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        }
    }
}
