//! Manifest producers and the frozen four-layer registry contract.

use crate::{
    limits::Limits,
    settings::resolve,
    source::{
        Catalog, Cause, Directory, Entry, Known, SourceTree, builtin, check,
        defaults::manifest_registry, frozen_rows,
    },
};
use maestro_settings::{Layer, Layers, Registry, SettingClass, SettingKind, Value, parse_flags};
use maestro_test_scratch::scratch_directory;
use std::{collections::BTreeSet, fs, io, path::PathBuf};

/// Synthetic backend fixture, checked through the production source pipeline.
const GRAPH: &str = include_str!("../../../../../tests/fixtures/catalog/backends/graphdb.toml");

/// Common defaults use the existing strict preference envelope.
const DEFAULTS: &str = "schema = \"maestro-preferences/1\"\nlanguage = \"fr\"\ntone = \"brief\"\n";

/// Build owned fixtures without exposing private paths or using another parser.
fn fixture(defaults: Option<&str>, graph: Option<&str>) -> PathBuf {
    let root = scratch_directory().unwrap();
    for (path, text) in [
        (
            "package.toml",
            include_str!("../../../../../tests/fixtures/catalog/source/package.toml"),
        ),
        (
            "core/package.toml",
            include_str!("../../../../../tests/fixtures/catalog/source/core-package.toml"),
        ),
    ] {
        fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        fs::write(root.join(path), text).unwrap();
    }
    for (path, text) in [
        ("settings/defaults.toml", defaults),
        ("core/backends/graphdb/config.toml", graph),
    ] {
        if let Some(text) = text {
            fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            fs::write(root.join(path), text).unwrap();
        }
    }
    root
}

/// Run the production checker over the synthetic directory.
fn catalog(defaults: Option<&str>, graph: Option<&str>) -> Result<Catalog, String> {
    let root = fixture(defaults, graph);
    let settings = Registry::built_in().unwrap();
    check(
        &Directory::new(&root),
        &builtin().unwrap(),
        &Limits::PRODUCTION,
        Known {
            rows: &frozen_rows(),
            settings: &settings,
            today: 0,
        },
    )
    .map_err(|error| error.to_string())
}

/// Feed the checked backend and optional common defaults into the sole S1 slot.
fn pinned_registry(defaults: Option<&str>, graph: Option<&str>) -> Registry {
    let catalog = catalog(None, graph).unwrap();
    manifest_registry(
        &Registry::built_in().unwrap(),
        &catalog.resources,
        defaults,
        &Limits::PRODUCTION,
    )
    .unwrap()
}

#[test]
fn manifest_default_producer_is_unique() {
    assert!(catalog(Some(DEFAULTS), Some(GRAPH)).is_ok());
    for (key, value) in [
        ("graph.engine", "\"ladybug\""),
        ("graphdb.max_num_threads", "2"),
        ("graphdb.buffer_pool_size", "268435456"),
        ("graphdb.max_db_size", "17179869184"),
    ] {
        let text = format!("{DEFAULTS}\n\"{key}\" = {value}\n");
        let error = catalog(Some(&text), Some(GRAPH)).unwrap_err();
        assert!(error.contains(key) && error.contains("producer"), "{error}");
    }
    let registry = pinned_registry(Some(DEFAULTS), Some(GRAPH));
    assert_eq!(
        registry.default_of("graph.engine"),
        Some(&Value::Text("ladybug".to_owned()))
    );
    assert_eq!(
        registry.default_of("graphdb.max_num_threads"),
        Some(&Value::Integer(2))
    );
    let changed = GRAPH
        .replace("268435456", "134217728")
        .replace("17179869184", "8589934592")
        .replace("max_num_threads = 2", "max_num_threads = 3");
    let changed = pinned_registry(None, Some(&changed));
    for (key, value) in [
        ("buffer_pool_size", 134_217_728),
        ("max_db_size", 8_589_934_592),
        ("max_num_threads", 3),
    ] {
        assert_eq!(
            changed.default_of(&format!("graphdb.{key}")),
            Some(&Value::Integer(value))
        );
    }
}

#[test]
fn four_layers_keep_frozen_semantics() {
    let registry = pinned_registry(Some(DEFAULTS), Some(GRAPH));
    let user = Layer::parse(
        &registry,
        "schema = \"maestro-preferences/1\"\nlanguage = \"es\"\n\"graphdb.max_num_threads\" = 8",
    )
    .unwrap();
    let workspace = Layer::parse(
        &registry,
        "schema = \"maestro-preferences/1\"\nlanguage = \"ja\"\n\"graphdb.max_num_threads\" = 16",
    )
    .unwrap();
    let flags = parse_flags(
        &registry,
        &[
            "language=en".to_owned(),
            "graphdb.max_num_threads=4".to_owned(),
        ],
    )
    .unwrap();
    let layers = Layers {
        user: Some((PathBuf::from("user.toml"), user)),
        project: Some((PathBuf::from("workspace.toml"), workspace)),
    };
    for (layers, flags, language, threads) in [
        (layers.clone(), flags.clone(), "en", 4),
        (layers.clone(), vec![], "ja", 8),
        (
            Layers {
                project: None,
                ..layers
            },
            vec![],
            "es",
            8,
        ),
        (Layers::default(), vec![], "fr", 2),
    ] {
        let resolved = resolve(
            &registry,
            &maestro_settings::resolve(&registry, &layers, &flags),
        );
        assert_eq!(resolved.text("language"), Some(language));
        assert_eq!(resolved.integer("graphdb.max_num_threads"), Some(threads));
    }
    let narrowed = Layers {
        project: Some((
            PathBuf::from("workspace.toml"),
            Layer::parse(
                &registry,
                "schema = \"maestro-preferences/1\"\n\"graphdb.max_num_threads\" = 1",
            )
            .unwrap(),
        )),
        ..Layers::default()
    };
    assert_eq!(
        resolve(
            &registry,
            &maestro_settings::resolve(&registry, &narrowed, &[])
        )
        .integer("graphdb.max_num_threads"),
        Some(1)
    );
    let widened = Layers {
        project: Some((
            PathBuf::from("workspace.toml"),
            Layer::parse(
                &registry,
                "schema = \"maestro-preferences/1\"\n\"graphdb.max_num_threads\" = 3",
            )
            .unwrap(),
        )),
        ..Layers::default()
    };
    assert_eq!(
        resolve(
            &registry,
            &maestro_settings::resolve(&registry, &widened, &[])
        )
        .integer("graphdb.max_num_threads"),
        Some(2)
    );
}

#[test]
fn masked_invalid_default_refuses() {
    for (key, value, reason) in [
        ("language", "\"en-US-extra\"", "language"),
        ("graphdb.max_num_threads", "65", "graphdb.max_num_threads"),
        ("provider_fallback", "false", "locked"),
        ("unknown", "true", "unknown"),
        ("graph.engine", "\"lbug\"", "graph.engine=ladybug"),
    ] {
        let text = format!("schema = \"maestro-preferences/1\"\n\"{key}\" = {value}\n");
        let error = catalog(Some(&text), None).unwrap_err();
        assert!(error.contains(reason), "{error}");
    }
    let registry = pinned_registry(Some(DEFAULTS), Some(GRAPH));
    for text in [
        "\"graphdb.max_db_size\" = 16777217",
        "provider_fallback = false",
        "\"graph.engine\" = \"lbug\"",
    ] {
        assert!(
            Layer::parse(
                &registry,
                &format!("schema = \"maestro-preferences/1\"\n{text}")
            )
            .is_err()
        );
    }
    assert!(parse_flags(&registry, &["graphdb.max_db_size=16777217".to_owned()]).is_err());
}

#[test]
fn graph_bounds_match_s2() {
    let registry = Registry::built_in().unwrap();
    for (key, min, max, default, power_of_two) in [
        (
            "buffer_pool_size",
            16_777_216,
            1_073_741_824,
            268_435_456,
            false,
        ),
        (
            "max_db_size",
            16_777_216,
            1_099_511_627_776,
            17_179_869_184,
            true,
        ),
        ("max_num_threads", 1, 64, 2, false),
    ] {
        let key = format!("graphdb.{key}");
        let descriptor = registry.get(&key).unwrap();
        assert_eq!(descriptor.class, SettingClass::Bounded);
        assert_eq!(
            descriptor.kind,
            SettingKind::Integer {
                min,
                max,
                off: false,
                power_of_two
            }
        );
        assert_eq!(registry.default_of(&key), Some(&Value::Integer(default)));
        for value in [min, max] {
            assert_eq!(
                descriptor.kind.parse_text(&value.to_string()).unwrap(),
                Value::Integer(value)
            );
        }
        for value in [min - 1, max + 1] {
            assert!(
                descriptor.kind.parse_text(&value.to_string()).is_err(),
                "{key}={value}"
            );
        }
    }
    assert_eq!(
        registry.default_of("graph.engine"),
        Some(&Value::Text("none".to_owned()))
    );
    let kind = &registry.get("graph.engine").unwrap().kind;
    assert!(kind.parse_text("ladybug").is_ok());
    assert!(
        kind.parse_text("lbug")
            .unwrap_err()
            .to_string()
            .contains("graph.engine=ladybug")
    );
}

#[test]
fn missing_manifest_defaults_keep_registered_defaults() {
    let registry = pinned_registry(None, None);
    assert_eq!(
        registry.default_of("language"),
        Some(&Value::Text("auto".to_owned()))
    );
    assert_eq!(
        registry.default_of("graph.engine"),
        Some(&Value::Text("none".to_owned()))
    );
    assert_eq!(
        registry.default_of("graphdb.max_num_threads"),
        Some(&Value::Integer(2))
    );
    assert!(catalog(Some("schema = \"maestro-preferences/1\""), None).is_ok());
}

#[test]
fn manifest_defaults_validate_whole_inputs_and_exact_limits() {
    let registry = Registry::built_in().unwrap();
    for text in [
        "language = \"en\"",
        "schema = \"other\"",
        "schema = \"maestro-preferences/1\"\nlanguage = \"en\"\nlanguage = \"fr\"",
        "schema = \"maestro-preferences/1\"\n[overrides]\n\"graphdb.max_db_size\" = 16777217",
    ] {
        assert!(catalog(Some(text), None).is_err(), "{text}");
    }
    let exact = Limits {
        source_file_bytes: DEFAULTS.len() as u64,
        source_depth: 1,
        ..Limits::PRODUCTION
    };
    assert!(manifest_registry(&registry, &[], Some(DEFAULTS), &exact).is_ok());
    let shorter = Limits {
        source_file_bytes: exact.source_file_bytes - 1,
        ..exact
    };
    assert!(manifest_registry(&registry, &[], Some(DEFAULTS), &shorter).is_err());
    let flat = "schema = \"maestro-preferences/1\"\n\"graphdb.max_num_threads\" = 2";
    let nested = "schema = \"maestro-preferences/1\"\n[graphdb]\nmax_num_threads = 2";
    let shallow = Limits {
        source_depth: 1,
        ..Limits::PRODUCTION
    };
    assert!(manifest_registry(&registry, &[], Some(flat), &shallow).is_ok());
    assert!(manifest_registry(&registry, &[], Some(nested), &shallow).is_err());
    assert!(manifest_registry(&registry, &[], Some(nested), &Limits::PRODUCTION).is_ok());
}

#[test]
fn masked_locked_preference_refuses_beneath_valid_flag() {
    let registry = pinned_registry(Some(DEFAULTS), None);
    let layers = Layers {
        user: Some((
            PathBuf::from("user.toml"),
            Layer::default().with("provider_fallback", Some(&Value::Flag(false))),
        )),
        ..Layers::default()
    };
    let flags = vec![maestro_settings::Flag {
        key: "provider_fallback".to_owned(),
        value: Value::Flag(false),
    }];
    let resolved = resolve(
        &registry,
        &maestro_settings::resolve(&registry, &layers, &flags),
    );
    assert!(resolved.get("provider_fallback").unwrap().is_err());
}

#[test]
fn manifest_defaults_refuse_non_utf8_and_names_only_registry() {
    let root = fixture(Some(DEFAULTS), None);
    let rows = frozen_rows();
    let names = BTreeSet::from(["language".to_owned(), "tone".to_owned()]);
    let kinds = builtin().unwrap();
    let known = Known {
        rows: &rows,
        settings: &names,
        today: 0,
    };
    let error = check(&Directory::new(&root), &kinds, &Limits::PRODUCTION, known).unwrap_err();
    assert!(error.to_string().contains("typed descriptors"));
    fs::write(root.join("settings/defaults.toml"), [0xff]).unwrap();
    let error = check(&Directory::new(&root), &kinds, &Limits::PRODUCTION, known).unwrap_err();
    assert!(error.to_string().contains("not UTF-8"));
    fs::remove_file(root.join("settings/defaults.toml")).unwrap();
    fs::create_dir(root.join("settings/defaults.toml")).unwrap();
    let error = check(&Directory::new(&root), &kinds, &Limits::PRODUCTION, known).unwrap_err();
    assert!(error.to_string().contains("defaults.toml"));
}

/// A source adapter that fails only the default read, on every host platform.
struct UnreadableDefaults(Directory);

impl SourceTree for UnreadableDefaults {
    fn list(&self, directory: &str) -> io::Result<Vec<Entry>> {
        self.0.list(directory)
    }
    fn read(&self, file: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        if file == "settings/defaults.toml" {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        self.0.read(file, max_bytes)
    }
}

#[test]
fn manifest_defaults_read_failure_is_fatal() {
    let root = fixture(Some(DEFAULTS), None);
    let settings = Registry::built_in().unwrap();
    let error = check(
        &UnreadableDefaults(Directory::new(&root)),
        &builtin().unwrap(),
        &Limits::PRODUCTION,
        Known {
            rows: &frozen_rows(),
            settings: &settings,
            today: 0,
        },
    )
    .unwrap_err();
    assert!(
        error
            .diagnostics
            .iter()
            .any(|problem| problem.path == "settings/defaults.toml"
                && problem.cause == Cause::Unreadable)
    );
}
