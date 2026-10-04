//! Preference drafts stay side-effect-free; CLI apply requires journal-backed trust.
use super::support::Home;
use maestro_catalog::policy::workspace::{CheckedTrust, JournalTrust, TrustBoundaries};
use maestro_catalog::{
    limits::Limits,
    settings::{FilePreferences, draft_preferences},
};
use maestro_settings::{FileLayers, Layer, LayerSource as _, Registry, Value};
use sha2::{Digest as _, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    str,
};

fn catalog() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/catalog/bootstrap/owner-local")
        .canonicalize()
        .unwrap()
}

fn preview(
    home: &Home,
    root: &Path,
    language: &str,
    tone: &str,
    apply: bool,
) -> super::support::Ended {
    let catalog = catalog();
    let language = format!("language={language}");
    let tone = format!("tone={tone}");
    let mut args = vec![
        "--json",
        "--set",
        &language,
        "--set",
        &tone,
        "--set",
        "routing_candidates=2",
        "init",
        "--yes",
        "--catalog-dir",
        catalog.to_str().unwrap(),
        "--preset",
        "base",
    ];
    if apply {
        args.push("--apply");
    }
    home.run_in(root, &args)
}

/// All deliverables except config remain identical; each independent C04 plan
/// binds the exact config digest without changing the bootstrap authoring lock.
#[test]
fn catalog_preferences_scripted_drafts_match_for_language_tone_matrix() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir(&root).unwrap();
    let mut baseline = None;
    for language in ["en", "fr", "es", "ja"] {
        for tone in ["brief", "normal", "detailed"] {
            let result = preview(&home, &root, language, tone, false);
            assert_eq!(result.code, Some(0), "{result:?}");
            let document: serde_json::Value = serde_json::from_str(&result.stdout).unwrap();
            let files = document["files"].clone();
            if let Some(expected) = &baseline {
                assert_eq!(&files, expected);
            } else {
                baseline = Some(files);
            }
            let entry = &document["preferences"]["files"]["entries"][0];
            assert_eq!(entry["path"], ".maestro/config.toml");
            let hex = entry["bytes"].as_str().unwrap();
            let bytes = decode_hex(hex);
            let digest = entry["digest"]
                .as_str()
                .unwrap()
                .strip_prefix("sha256:")
                .unwrap();
            assert_eq!(decode_hex(digest), Sha256::digest(&bytes).as_slice());
            let draft = draft_preferences(
                &root,
                &FilePreferences::new(&home.config(), &root),
                &[
                    format!("language={language}"),
                    format!("tone={tone}"),
                    "routing_candidates=2".into(),
                ],
                &Limits::PRODUCTION,
                &CheckedTrust::new(
                    &JournalTrust::optional(None),
                    &TrustBoundaries::new(home.root(), &[]).unwrap(),
                ),
            )
            .unwrap();
            assert_eq!(bytes, draft.file.bytes);
            let text = String::from_utf8(bytes).unwrap();
            let registry = Registry::built_in().unwrap();
            let layer = Layer::parse_preferences(&registry, &text, 1024, 4).unwrap();
            assert_eq!(layer.get("language"), Some(&Value::Text(language.into())));
            assert_eq!(layer.get("tone"), Some(&Value::Text(tone.into())));
            assert_eq!(layer.get("routing_candidates"), Some(&Value::Integer(2)));
            assert!(!root.join(".maestro").exists());
            assert!(!root.join(".github").exists());
        }
    }
}

#[test]
fn catalog_preferences_apply_refuses_without_touching_root_ancestor_or_authority() {
    let home = Home::bare();
    let root = home.root().join("parent/project");
    fs::create_dir_all(&root).unwrap();
    let ancestor = root.parent().unwrap().join(".maestro/config.toml");
    fs::create_dir(ancestor.parent().unwrap()).unwrap();
    fs::write(
        &ancestor,
        b"schema = 'maestro-preferences/1'\n# ancestor unchanged\n",
    )
    .unwrap();
    home.configure("[access]\nread = ['workspace/default']\n");
    let authority = fs::read(home.config().join("config.toml")).unwrap();
    let result = preview(&home, &root, "FR", "brief", true);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(result.stderr.contains("maestro trust add"), "{result:?}");
    assert!(!root.join(".maestro").exists());
    assert!(!root.join(".github").exists());
    assert_eq!(
        fs::read(&ancestor).unwrap(),
        b"schema = 'maestro-preferences/1'\n# ancestor unchanged\n"
    );
    assert_eq!(
        fs::read(home.config().join("config.toml")).unwrap(),
        authority
    );
}

#[test]
fn catalog_preferences_reads_and_existing_config_commands_edit_and_explain_overrides() {
    let home = Home::bare();
    let root = home.root().join("project");
    fs::create_dir_all(root.join(".maestro")).unwrap();
    let path = root.join(".maestro/config.toml");
    for original in [
        concat!(
            "schema = 'maestro-preferences/1'\nlanguage = 'fr'\nrouting_candidates = 2\n",
            "[evidence]\nexpansion = 'parent_chain'\nparent_chain_order = 'off'\n"
        ),
        concat!(
            "schema = 'maestro-preferences/1'\nlanguage = 'fr'\n[overrides]\n",
            "routing_candidates = 2\n[overrides.evidence]\n",
            "expansion = 'parent_chain'\nparent_chain_order = 'off'\n"
        ),
    ] {
        fs::write(&path, original).unwrap();
        let registry = Registry::built_in().unwrap();
        assert_eq!(
            FileLayers::new(&home.root().join("unused"), Some(path.clone()))
                .layers(&registry)
                .unwrap()
                .project
                .unwrap()
                .1
                .get("routing_candidates"),
            Some(&Value::Integer(2))
        );
        let result = preview(&home, &root, "fr", "brief", false);
        assert_eq!(result.code, Some(2), "{result:?}");
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        let explained = home.run_in(&root, &["config", "explain", "routing_candidates"]);
        assert_eq!(explained.code, Some(0), "{explained:?}");
        assert!(explained.stdout.contains("project"), "{explained:?}");
        let set = home.run_in(
            &root,
            &["config", "set", "routing_candidates", "1", "--project"],
        );
        assert_eq!(set.code, Some(0), "{set:?}");
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("routing_candidates = 1")
        );
        let unset = home.run_in(
            &root,
            &["config", "unset", "routing_candidates", "--project"],
        );
        assert_eq!(unset.code, Some(0), "{unset:?}");
        assert!(
            !fs::read_to_string(&path)
                .unwrap()
                .contains("routing_candidates")
        );
    }
}

fn decode_hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
