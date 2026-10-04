//! Runtime inputs share the captured closure and C04 replay refusal.
use super::{
    inventory::Fixture,
    support::{apply, preview},
};
use crate::{bootstrap::AreaInventories, files::digest};
use std::{collections::BTreeSet, fs};

/// Every non-resource placement and all three immutable core backend bases.
const INPUTS: [&str; 4] = [
    "settings/defaults.toml",
    "core/backends/graphdb/config.toml",
    "core/backends/vectordb/config.toml",
    "core/backends/mcp/config.toml",
];

/// Fixture adapters are data only; explicit none requires no graph engine.
fn compiled() -> BTreeSet<String> {
    BTreeSet::from(["qdrant".to_owned(), "knowledge".to_owned()])
}

/// Use the production runtime admission seam with the synthetic compiled set.
fn port(fixture: &Fixture) -> AreaInventories {
    fixture.port().unwrap().with_compiled_backends(compiled())
}

/// Synthetic defaults and explicit disabled graph require no live adapter.
fn inputs(fixture: &Fixture) {
    for (path, text) in [
        (
            "settings/defaults.toml",
            "schema = 'maestro-preferences/1'\nlanguage = 'fr'\ntone = 'brief'\n".to_owned(),
        ),
        (
            "core/backends/graphdb/config.toml",
            include_str!("../../../../../tests/fixtures/catalog/backends/graphdb.toml")
                .replace("ladybug", "none"),
        ),
        (
            "core/backends/vectordb/config.toml",
            include_str!("../../../../../tests/fixtures/catalog/backends/vectordb.toml").to_owned(),
        ),
        (
            "core/backends/mcp/config.toml",
            include_str!("../../../../../tests/fixtures/catalog/backends/mcp.toml").to_owned(),
        ),
    ] {
        fs::create_dir_all(fixture.catalog.join(path).parent().unwrap()).unwrap();
        fs::write(fixture.catalog.join(path), text).unwrap();
    }
}

#[test]
fn nonresource_input_change_requires_preview() {
    for path in INPUTS {
        let fixture = Fixture::new();
        inputs(&fixture);
        let provider = port(&fixture);
        let proposal = preview(&fixture.project, &provider, &["base".into()]).unwrap();
        let original = fs::read(fixture.catalog.join(path)).unwrap();
        fs::write(
            fixture.catalog.join(path),
            [original.as_slice(), b"\n"].concat(),
        )
        .unwrap();
        let error = apply(&fixture.project, &proposal).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("input changed; run preview again"),
            "{error}"
        );
        assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
        fs::write(fixture.catalog.join(path), &original).unwrap();
        apply(&fixture.project, &proposal).unwrap();
        let lock: serde_json::Value = serde_json::from_slice(
            &fs::read(fixture.project.join(".maestro/authoring.lock.json")).unwrap(),
        )
        .unwrap();
        let source = lock["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source| source["path"] == path)
            .unwrap();
        assert_eq!(source["sha256"], digest(&original));
        let identity = if path == INPUTS[0] {
            "package:common".to_owned()
        } else {
            format!("backend:core/{}", path.split('/').nth(2).unwrap())
        };
        assert_eq!(source["id"], identity);
        fs::write(
            fixture.catalog.join(path),
            [original.as_slice(), b"\n"].concat(),
        )
        .unwrap();
        let error = preview(&fixture.project, &port(&fixture), &["base".into()]).unwrap_err();
        assert!(error.contains("preview again"), "{error}");
    }
}

#[test]
fn session_keeps_admitted_defaults() {
    use crate::{
        files::tests::support::with_trust,
        limits::Limits,
        settings::{SessionPreferences, draft_preferences},
    };
    let fixture = Fixture::new();
    inputs(&fixture);
    let proposal = preview(&fixture.project, &port(&fixture), &["base".into()]).unwrap();
    apply(&fixture.project, &proposal).unwrap();
    with_trust(&fixture.project, |trust| {
        let snapshot = SessionPreferences::load(
            &fixture.root.join("user"),
            Some(&fixture.project),
            Some(&fixture.root),
            trust,
            &Limits::PRODUCTION,
        )
        .unwrap()
        .admit_defaults(trust, &compiled(), &Limits::PRODUCTION)
        .unwrap();
        let draft = draft_preferences(&fixture.project, &snapshot, &[], &Limits::PRODUCTION, trust)
            .unwrap();
        assert!(
            String::from_utf8(draft.file.bytes)
                .unwrap()
                .contains("language = \"fr\"")
        );
    });
}

#[test]
fn newly_added_nonresource_input_requires_preview() {
    for path in INPUTS {
        let fixture = Fixture::new();
        let proposal = preview(&fixture.project, &port(&fixture), &["base".into()]).unwrap();
        inputs(&fixture);
        for other in INPUTS.into_iter().filter(|other| *other != path) {
            fs::remove_file(fixture.catalog.join(other)).unwrap();
        }
        assert!(
            apply(&fixture.project, &proposal).is_err(),
            "new input: {path}"
        );
        assert_eq!(fs::read_dir(&fixture.project).unwrap().count(), 0);
    }
}
