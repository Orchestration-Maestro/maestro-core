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
