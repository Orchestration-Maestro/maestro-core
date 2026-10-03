//! Strict init drafts use the same S1 file/parser API as all preference consumers.
use crate::{
    files::tests::support::with_trust,
    limits::Limits,
    settings::{self, FilePreferences, PreferencesDraft, WorkspacePreferences},
};
use maestro_settings::{Layer, Layers, Registry, Value};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process, str,
    time::{SystemTime, UNIX_EPOCH},
};

/// An isolated init root, removed on drop.
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let root = env::temp_dir().join(format!(
            "maestro-c05a-{}-{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn source(&self) -> FilePreferences {
        FilePreferences::new(&self.0.join("user"), &self.0)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

#[test]
fn strict_files_allow_partial_preferences_and_canonicalize_supported_tags() {
    let root = Root::new();
    let registry = Registry::built_in().unwrap();
    fs::create_dir(root.0.join(".maestro")).unwrap();
    for (input, expected) in [
        ("EN", "en"),
        ("FR", "fr"),
        ("ES", "es"),
        ("JA", "ja"),
        ("ZH-hant-tw", "zh-Hant-TW"),
        ("es-419", "es-419"),
        ("SR-latn", "sr-Latn"),
    ] {
        fs::write(
            root.0.join(".maestro/config.toml"),
            format!("schema = 'maestro-preferences/1'\nlanguage = '{input}'"),
        )
        .unwrap();
        let layers = root
            .source()
            .layers(&registry, &Limits::PRODUCTION)
            .unwrap();
        assert_eq!(
            layers.project.unwrap().1.get("language"),
            Some(&Value::Text(expected.into()))
        );
    }
    fs::write(
        root.0.join(".maestro/config.toml"),
        "schema = 'maestro-preferences/1'\ntone = 'normal'\n",
    )
    .unwrap();
    assert!(
        root.source()
            .layers(&registry, &Limits::PRODUCTION)
            .unwrap()
            .project
            .unwrap()
            .1
            .get("language")
            .is_none()
    );
    fs::remove_file(root.0.join(".maestro/config.toml")).unwrap();
    let draft = draft_preferences(&root.0, &root.source(), &[], &Limits::PRODUCTION).unwrap();
    let text = str::from_utf8(&draft.file.bytes).unwrap();
    assert_eq!(
        Layer::parse_preferences(&registry, text, 1024, 4)
            .unwrap()
            .get("language"),
        Some(&Value::Text("en".into()))
    );
    assert!(!root.0.join(".maestro/config.toml").exists());
}

#[test]
fn both_files_refuse_authority_bad_tags_types_duplicates_and_versions() {
    let root = Root::new();
    fs::create_dir(root.0.join("user")).unwrap();
    fs::create_dir(root.0.join(".maestro")).unwrap();
    let registry = Registry::built_in().unwrap();
    for body in [
        "language = 'sl-rozaj'",
        "language = 'en-u-ca-gregory'",
        "language = 'en-x-test'",
        "language = 'i-klingon'",
        "language = 'en-GB-oed'",
        "language = 'x-private'",
        "language = 'éé'",
        "language = 'en--US'",
        "language = 12",
        "tone = 'loud'",
        "tone = true",
        "updates = 'yes'",
        "updates = 1",
        "tone = 'brief'\ntone = 'normal'",
        "tone = 'brief'\n[overrides]\ntone = 'normal'",
        "[access]\nread = []",
        "identity = 'owner'",
        "hooks = []",
        "secrets = []",
        "trust = true",
        "paths = []",
        "receipts = []",
        "[overrides]\nunknown = 1",
        "[overrides]\nrouting_candidates = 4",
        "[overrides]\nraw_prompt_logging = false",
    ] {
        for path in [
            root.0.join("user/preferences.toml"),
            root.0.join(".maestro/config.toml"),
        ] {
            fs::write(&path, format!("schema = 'maestro-preferences/1'\n{body}")).unwrap();
            assert!(
                root.source()
                    .layers(&registry, &Limits::PRODUCTION)
                    .is_err(),
                "{body}"
            );
            fs::remove_file(&path).unwrap();
        }
    }
    for schema in ["", "schema = 'maestro-preferences/2'", "schema = 1"] {
        fs::write(root.0.join("user/preferences.toml"), schema).unwrap();
        assert!(
            root.source()
                .layers(&registry, &Limits::PRODUCTION)
                .is_err()
        );
    }
}

#[test]
fn file_adapter_enforces_exact_and_one_past_byte_and_depth_bounds() {
    let root = Root::new();
    let registry = Registry::built_in().unwrap();
    fs::create_dir(root.0.join("user")).unwrap();
    fs::create_dir(root.0.join(".maestro")).unwrap();
    let text = "schema = 'maestro-preferences/1'\n[overrides]\nrouting_candidates = 2\n";
    let exact = Limits {
        source_file_bytes: text.len() as u64,
        source_depth: 2,
        ..Limits::PRODUCTION
    };
    for path in [
        root.0.join("user/preferences.toml"),
        root.0.join(".maestro/config.toml"),
    ] {
        fs::write(&path, text).unwrap();
        assert!(root.source().layers(&registry, &exact).is_ok());
        assert!(
            root.source()
                .layers(
                    &registry,
                    &Limits {
                        source_file_bytes: exact.source_file_bytes - 1,
                        ..exact
                    }
                )
                .is_err()
        );
        assert!(
            root.source()
                .layers(
                    &registry,
                    &Limits {
                        source_depth: 1,
                        ..exact
                    }
                )
                .is_err()
        );
        fs::remove_file(&path).unwrap();
    }
}

#[test]
fn fixture_layers_and_init_updates_only_narrow_and_plan_local_owned_bytes() {
    let root = Root::new();
    fs::create_dir(root.0.join("user")).unwrap();
    fs::write(
        root.0.join("user/preferences.toml"),
        include_str!("../../../../../tests/fixtures/catalog/settings/user-preferences.toml"),
    )
    .unwrap();
    fs::create_dir(root.0.join(".maestro")).unwrap();
    fs::write(
        root.0.join(".maestro/config.toml"),
        include_str!("../../../../../tests/fixtures/catalog/settings/workspace.toml"),
    )
    .unwrap();
    let layers = root
        .source()
        .layers(&Registry::built_in().unwrap(), &Limits::PRODUCTION)
        .unwrap();
    assert_eq!(
        layers.user.unwrap().1.get("updates"),
        Some(&Value::Text("auto".into()))
    );
    fs::remove_file(root.0.join(".maestro/config.toml")).unwrap();
    for tone in ["brief", "normal", "detailed"] {
        for updates in ["off", "propose", "auto"] {
            let draft = draft_preferences(
                &root.0,
                &root.source(),
                &[
                    format!("tone={tone}"),
                    format!("updates={updates}"),
                    "routing_candidates=1".into(),
                ],
                &Limits::PRODUCTION,
            )
            .unwrap();
            assert_eq!(draft.file.path, ".maestro/config.toml");
            let parsed = Layer::parse_preferences(
                &Registry::built_in().unwrap(),
                str::from_utf8(&draft.file.bytes).unwrap(),
                1024,
                4,
            )
            .unwrap();
            assert_eq!(
                parsed.get("updates"),
                Some(&Value::Text(
                    if updates == "off" { "off" } else { "propose" }.into()
                ))
            );
            assert_eq!(parsed.get("tone"), Some(&Value::Text(tone.into())));
            assert_eq!(parsed.get("routing_candidates"), Some(&Value::Integer(1)));
        }
    }
    fs::write(
        root.0.join("user/preferences.toml"),
        "schema = 'maestro-preferences/1'\nupdates = 'propose'\n",
    )
    .unwrap();
    let draft = draft_preferences(
        &root.0,
        &root.source(),
        &["updates=auto".into()],
        &Limits::PRODUCTION,
    )
    .unwrap();
    assert!(!draft.diagnostics.is_empty());
    let original = draft.file.bytes.clone();
    fs::write(
        root.0.join(".maestro/config.toml"),
        b"schema = 'maestro-preferences/1'\n# user edit\n",
    )
    .unwrap();
    assert!(draft_preferences(&root.0, &root.source(), &[], &Limits::PRODUCTION).is_err());
    assert!(
        fs::read(root.0.join(".maestro/config.toml"))
            .unwrap()
            .ends_with(b"# user edit\n")
    );
    assert!(!original.is_empty());
}

/// Alternate storage still supplies typed S1 layers; consumers need no file knowledge.
struct MemoryPreferences;
impl WorkspacePreferences for MemoryPreferences {
    fn layers(&self, registry: &Registry, limits: &Limits) -> Result<Layers, String> {
        let layer = Layer::parse_preferences(
            registry,
            "schema = 'maestro-preferences/1'\nlanguage = 'JA'\nupdates = 'off'\n",
            limits.source_file_bytes,
            limits.source_depth,
        )
        .map_err(|error| error.to_string())?;
        Ok(Layers {
            user: Some((PathBuf::from("preferences.toml"), layer)),
            project: None,
        })
    }
}

#[test]
fn draft_choices_are_typed_and_the_source_port_is_replaceable() {
    let root = Root::new();
    let draft = draft_preferences(
        &root.0,
        &MemoryPreferences,
        &["updates=propose".into()],
        &Limits::PRODUCTION,
    )
    .unwrap();
    let text = str::from_utf8(&draft.file.bytes).unwrap();
    assert_eq!(
        text,
        concat!(
            "schema = \"maestro-preferences/1\"\nlanguage = \"ja\"\n",
            "tone = \"normal\"\nupdates = \"off\"\n"
        )
    );
    assert!(!draft.diagnostics.is_empty());
    for choices in [
        vec!["language=auto"],
        vec!["language=zh-TW-Hant"],
        vec!["updates=unknown"],
        vec!["tone=normal", "tone=brief"],
        vec!["routing_candidates=no"],
        vec!["access.read=x"],
        vec!["discovered_executable_hooks=true"],
    ] {
        let choices: Vec<_> = choices.into_iter().map(str::to_owned).collect();
        assert!(
            draft_preferences(&root.0, &MemoryPreferences, &choices, &Limits::PRODUCTION).is_err()
        );
    }
}

/// The original draft contracts now supply a checked synthetic authority port.
fn draft_preferences(
    root: &Path,
    source: &dyn WorkspacePreferences,
    choices: &[String],
    limits: &Limits,
) -> Result<PreferencesDraft, String> {
    with_trust(root, |trust| {
        settings::draft_preferences(root, source, choices, limits, trust)
    })
}

#[test]
fn preferences_only_sources_have_no_admitted_lock_identity() {
    let source = FilePreferences::new(Path::new("config"), Path::new("project"));
    assert_eq!(
        source.frozen_lock(),
        None,
        "preferences-only source cannot invent a durable graph input pin"
    );
}
