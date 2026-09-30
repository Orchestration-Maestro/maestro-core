//! Additive bounded preferences and legacy compatibility through the shared parser.
use crate::{
    FileLayers, Layer, LayerSource as _, Registry, Value, set_in_document, unset_in_document,
};
use maestro_test_scratch::scratch_directory;
use std::fs;

const SCHEMA: &str = "schema = \"maestro-preferences/1\"\n";

#[test]
fn file_reads_and_edits_round_trip_both_forms_without_duplicates() {
    let registry = Registry::built_in().unwrap();
    let scratch = scratch_directory().unwrap();
    for body in [
        "[evidence]\nexpansion = 'full_section'\nparent_chain_order = 'off'\n",
        "[overrides.evidence]\nexpansion = 'full_section'\nparent_chain_order = 'off'\n",
    ] {
        let text = format!("{SCHEMA}{body}");
        let path = scratch.join("preferences.toml");
        fs::write(&path, &text).unwrap();
        let source = FileLayers::new(&scratch, None);
        let layers = source.layers(&registry).unwrap();
        assert_eq!(
            layers.user.unwrap().1.get("evidence.expansion"),
            Some(&Value::Text("full_section".into()))
        );
        let edited = set_in_document(
            &registry,
            Some(&text),
            "evidence.expansion",
            &Value::Text("parent_chain".into()),
        )
        .unwrap();
        let added = set_in_document(
            &registry,
            Some(&edited),
            "routing_candidates",
            &Value::Integer(2),
        )
        .unwrap();
        let layer = FileLayers::new(&scratch, None);
        fs::write(&path, &added).unwrap();
        let layer = layer.layers(&registry).unwrap().user.unwrap().1;
        assert_eq!(
            layer.get("evidence.expansion"),
            Some(&Value::Text("parent_chain".into()))
        );
        assert_eq!(layer.get("routing_candidates"), Some(&Value::Integer(2)));
        let unset = unset_in_document(&registry, &added, "evidence.expansion")
            .unwrap()
            .unwrap();
        assert!(
            {
                fs::write(&path, &unset).unwrap();
                FileLayers::new(&scratch, None)
                    .layers(&registry)
                    .unwrap()
                    .user
                    .unwrap()
                    .1
            }
            .get("evidence.expansion")
            .is_none()
        );
        if body.starts_with("[overrides") {
            assert!(added.contains("[overrides]"));
        }
    }
    for (key, first, second) in [("tone", "brief", "normal"), ("language", "en", "fr")] {
        let duplicate = format!("{SCHEMA}{key} = '{first}'\n[overrides]\n{key} = '{second}'\n");
        fs::write(scratch.join("preferences.toml"), &duplicate).unwrap();
        assert!(
            FileLayers::new(&scratch, None).layers(&registry).is_err(),
            "{key}"
        );
        assert!(
            set_in_document(&registry, Some(&duplicate), key, &Value::Text(first.into())).is_err(),
            "{key}"
        );
        assert!(
            unset_in_document(&registry, &duplicate, key).is_err(),
            "{key}"
        );
    }
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn bounded_preferences_use_registry_keys_in_overrides() {
    let registry = Registry::built_in().unwrap();
    let layer = Layer::parse_preferences(
        &registry,
        &format!("{SCHEMA}[overrides]\nmodel_profile = 'fast'\nrouting_candidates = 2\n"),
        1024,
        2,
    )
    .unwrap();
    assert_eq!(
        layer.get("model_profile"),
        Some(&Value::Text("fast".into()))
    );
    assert_eq!(layer.get("routing_candidates"), Some(&Value::Integer(2)));
    for body in [
        "[overrides]\nunknown = 1",
        "[overrides]\nraw_prompt_logging = false",
        "tone = 'brief'\n[overrides]\ntone = 'normal'",
    ] {
        assert!(
            Layer::parse_preferences(&registry, &format!("{SCHEMA}{body}"), 1024, 4).is_err(),
            "{body}"
        );
    }
}

#[test]
fn bounded_preferences_refuse_unknown_empty_tables() {
    let registry = Registry::built_in().unwrap();
    for (body, key) in [
        ("[overrides.\"tone.extra\"]\n", "tone.extra"),
        ("[overrides.evidence.unknown]\n", "evidence.unknown"),
        ("[\"tone.extra\"]\n", "tone.extra"),
    ] {
        assert_eq!(
            Layer::parse_preferences(&registry, &format!("{SCHEMA}{body}"), 1024, 4),
            Err(crate::LayerError::UnknownKey(key.into())),
            "{body}"
        );
    }
    for body in ["[overrides.evidence]\n", "[evidence]\n"] {
        assert!(
            Layer::parse_preferences(&registry, &format!("{SCHEMA}{body}"), 1024, 4)
                .unwrap()
                .is_empty(),
            "{body}"
        );
    }
}

#[test]
fn bounded_preferences_exact_and_one_past_limits() {
    let registry = Registry::built_in().unwrap();
    let text = format!("{SCHEMA}[overrides]\nsearch.routes.dense = false\n");
    let bytes = u64::try_from(text.len()).unwrap();
    assert!(Layer::parse_preferences(&registry, &text, bytes, 4).is_ok());
    assert!(Layer::parse_preferences(&registry, &text, bytes - 1, 4).is_err());
    assert!(Layer::parse_preferences(&registry, &text, bytes, 3).is_err());
    assert!(Layer::parse_preferences(&registry, SCHEMA, 1024, 0).is_err());
}
