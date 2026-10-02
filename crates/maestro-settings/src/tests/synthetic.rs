//! Adding a setting is one descriptor: a synthetic one, appended to the
//! built-in registry, is parsed from files and flags, resolved with its
//! source, edited in place and serialized, with no code of its own.

use crate::{
    BUILT_IN, Layer, LayerName, Layers, Registry, SettingClass, SettingDescriptor, SettingKind,
    Source, Value, parse_flags, resolve, set_in_document, unset_in_document,
};
use std::{borrow::Cow, path::PathBuf};

/// A setting no lane has landed: the reply cap, as its descriptor.
const REPLY_CAP: SettingDescriptor = SettingDescriptor {
    key: Cow::Borrowed("ask.reply.cap_tokens"),
    kind: SettingKind::Integer {
        min: 64,
        max: 4096,
        off: true,
    },
    default: Cow::Borrowed("off"),
    description: Cow::Borrowed("A synthetic cap on each answerer reply."),
    class: SettingClass::Free,
    standard_only: false,
};

#[test]
fn a_synthetic_descriptor_is_a_setting_everywhere() {
    let mut descriptors = BUILT_IN.to_vec();
    descriptors.push(REPLY_CAP);
    let registry = Registry::new(&descriptors).unwrap();

    let schema = "schema = \"maestro-preferences/1\"\n";
    let user = Layer::parse(
        &registry,
        &format!("{schema}[ask.reply]\ncap_tokens = 512\n"),
    )
    .unwrap();
    let project_text =
        set_in_document(&registry, None, &REPLY_CAP.key, &Value::Integer(256)).unwrap();
    assert_eq!(
        project_text,
        format!("{schema}\n[ask.reply]\ncap_tokens = 256\n")
    );
    let project = Layer::parse(&registry, &project_text).unwrap();
    let layers = Layers {
        user: Some((PathBuf::from("user.toml"), user)),
        project: Some((PathBuf::from("project.toml"), project)),
    };

    let resolved = resolve(&registry, &layers, &[]);
    let setting = resolved.get(&REPLY_CAP.key).unwrap();
    assert_eq!(setting.value, Value::Integer(256));
    assert_eq!(
        setting.source,
        Source::File {
            layer: LayerName::Project,
            path: PathBuf::from("project.toml")
        }
    );
    let flags = parse_flags(&registry, &["ask.reply.cap_tokens=off".to_owned()]).unwrap();
    assert!(resolve(&registry, &layers, &flags).is_off(&REPLY_CAP.key));
    assert_eq!(
        resolve(&registry, &Layers::default(), &[])
            .get(&REPLY_CAP.key)
            .unwrap()
            .source,
        Source::Default
    );
    assert_eq!(
        unset_in_document(&registry, &project_text, &REPLY_CAP.key).unwrap(),
        Some(format!("{schema}\n[ask.reply]\n"))
    );
    assert!(
        Layer::parse(
            &registry,
            &format!("{schema}[ask.reply]\ncap_tokens = 9999\n")
        )
        .unwrap_err()
        .to_string()
        .starts_with("ask.reply.cap_tokens: expected a whole number from 64 to 4096")
    );
    let json = serde_json::to_string(&descriptors).unwrap();
    let read: Vec<SettingDescriptor> = serde_json::from_str(&json).unwrap();
    assert_eq!(read, descriptors);
}
