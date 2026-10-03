//! Strict embedded data and non-recursive interpolation contracts.
use super::messages::{BUILT_INS, Interface, MessageKey, Translation, interpolate};
use serde_json::Value;

#[test]
fn catalog_presentation_key_set_parity_and_unknown_keys_refuse() {
    for (_, source) in BUILT_INS {
        assert!(Translation::parse(source).is_ok());
        let data: Value = serde_json::from_str(source).unwrap();
        let keys: Vec<_> = data.as_object().unwrap().keys().cloned().collect();
        for key in keys {
            let mut missing = data.clone();
            missing.as_object_mut().unwrap().remove(&key);
            assert!(
                Translation::parse(&missing.to_string()).is_err(),
                "missing {key}"
            );
        }
        let mut unknown = data.clone();
        unknown["unknown_key"] = Value::String("Unknown".to_owned());
        assert!(Translation::parse(&unknown.to_string()).is_err());
        let duplicate = source.replacen('{', "{\"interface_fallback\":\"Duplicate\",", 1);
        assert!(Translation::parse(&duplicate).is_err());
        let mut wrong_type = data;
        wrong_type["interface_fallback"] = Value::Bool(true);
        assert!(Translation::parse(&wrong_type.to_string()).is_err());
    }
}

#[test]
fn catalog_presentation_placeholder_parity_refuses_malformed_templates() {
    for (_, source) in BUILT_INS {
        for replacement in [
            "No placeholder",
            "Wrong {tag}",
            "Extra {language} {extra}",
            "Unclosed {language",
            "Stray language}",
            "Nested {{language}}",
            "Invalid {language-tag}",
        ] {
            let mut data: Value = serde_json::from_str(source).unwrap();
            data["interface_fallback"] = Value::String(replacement.to_owned());
            assert!(
                Translation::parse(&data.to_string()).is_err(),
                "{replacement}"
            );
        }
    }
}

#[test]
fn catalog_presentation_interpolation_preserves_data_without_recursive_translation() {
    for language in ["en", "fr", "es", "ja"] {
        let interface = Interface::select(language).unwrap();
        let template = interface.template(MessageKey::InitTrustCommand);
        let data = "\"/équipe/{language}/$literal\"";
        let rendered = interpolate(template, &[("path", data)]).unwrap();
        assert!(rendered.contains(&format!("maestro trust add {data} --confirm-path {data}")));
        assert!(interpolate(template, &[]).is_err());
        assert!(interpolate(template, &[("wrong", data)]).is_err());
        assert!(interpolate(template, &[("path", data), ("path", data)]).is_err());
        assert!(interpolate(template, &[("path", data), ("extra", data)]).is_err());
    }
}

#[test]
fn catalog_presentation_selection_is_deterministic_and_fallback_is_english() {
    let english = Interface::select("en").unwrap();
    for language in ["en", "fr", "es", "ja", "zh-Hant-TW", "auto"] {
        let interface = Interface::select(language).unwrap();
        assert_eq!(
            interface.fallback(),
            !["en", "fr", "es", "auto"].contains(&language)
        );
        let first = interface.template(MessageKey::InitPreferencesWritten);
        assert_eq!(
            first,
            interface.template(MessageKey::InitPreferencesWritten)
        );
        if interface.fallback() || language == "auto" {
            assert_eq!(first, english.template(MessageKey::InitPreferencesWritten));
        }
    }
}

#[test]
fn catalog_presentation_translation_golden_covers_every_key_and_language() {
    use std::collections::{BTreeMap, BTreeSet};

    let path = [("path", "/synthetic/équipe/{literal}")];
    let messages = [
        (MessageKey::FlowWorkspace, "flow_workspace", &[][..]),
        (MessageKey::FlowLanguage, "flow_language", &[][..]),
        (MessageKey::FlowTone, "flow_tone", &[][..]),
        (MessageKey::FlowSettings, "flow_settings", &[][..]),
        (MessageKey::FlowReview, "flow_review", &[][..]),
        (MessageKey::FlowEditorPrompt, "flow_editor_prompt", &[][..]),
        (MessageKey::FlowApplyPrompt, "flow_apply_prompt", &[][..]),
        (
            MessageKey::FlowPreviewPrompt,
            "flow_preview_prompt",
            &[][..],
        ),
        (
            MessageKey::InterfaceFallback,
            "interface_fallback",
            &[("language", "ja")][..],
        ),
        (
            MessageKey::InitUntrusted,
            "init_untrusted",
            &[("instruction", "synthetic instruction")],
        ),
        (MessageKey::InitTrustCommand, "init_trust_command", &path),
        (MessageKey::InitTrustData, "init_trust_data", &path),
        (
            MessageKey::InitConfirm,
            "init_confirm",
            &[
                ("failure", "synthetic failure"),
                ("instruction", "synthetic instruction"),
            ],
        ),
        (MessageKey::InitApprovePrompt, "init_approve_prompt", &path),
        (
            MessageKey::InitConfirmCommand,
            "init_confirm_command",
            &path,
        ),
        (MessageKey::InitConfirmData, "init_confirm_data", &path),
        (
            MessageKey::InitPreferencesWritten,
            "init_preferences_written",
            &[],
        ),
        (
            MessageKey::InitPreferencesDeclined,
            "init_preferences_declined",
            &[],
        ),
    ];
    let migrated = super::inventory_tests::migrated_keys();
    let mut rendered = BTreeMap::new();
    for (language, source) in BUILT_INS {
        let interface = Interface::select(language).unwrap();
        let wording: BTreeMap<_, _> = messages
            .iter()
            .map(|(key, name, values)| {
                (
                    *name,
                    interpolate(interface.template(*key), values).unwrap(),
                )
            })
            .collect();
        let translation: Value = serde_json::from_str(source).unwrap();
        assert_eq!(
            wording.keys().copied().collect::<BTreeSet<_>>(),
            translation
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .filter(|name| !migrated.iter().any(|(_, migrated)| migrated == name))
                .collect::<BTreeSet<_>>(),
            "golden must render every key in {language}"
        );
        rendered.insert(*language, wording);
    }
    assert_eq!(
        format!("{}\n", serde_json::to_string_pretty(&rendered).unwrap()),
        include_str!("languages/rendered.golden.json")
    );
}
