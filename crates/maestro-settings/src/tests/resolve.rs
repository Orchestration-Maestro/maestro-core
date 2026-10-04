//! Resolution: per key, an explicit `--set`, then the project file, then the
//! user file, then the default, each value with the layer that set it.

use crate::{
    Layer, LayerName, Layers, Registry, SettingClass, SettingDescriptor, SettingKind, Source,
    Value, parse_flags, resolve,
};
use std::{borrow::Cow, path::PathBuf};

/// The built-in registry.
fn registry() -> Registry {
    Registry::built_in().unwrap()
}

/// The layer of the preferences `body`.
fn layer(body: &str) -> Layer {
    Layer::parse(
        &registry(),
        &format!("schema = \"maestro-preferences/1\"\n{body}"),
    )
    .unwrap()
}

/// The user file's source.
fn user() -> Source {
    Source::File {
        layer: LayerName::User,
        path: PathBuf::from("user.toml"),
    }
}

/// The project file's source.
fn project() -> Source {
    Source::File {
        layer: LayerName::Project,
        path: PathBuf::from("project.toml"),
    }
}

/// Layers of the user `user_body` and the project `project_body`.
fn layers(user_body: &str, project_body: &str) -> Layers {
    Layers {
        user: Some((PathBuf::from("user.toml"), layer(user_body))),
        project: Some((PathBuf::from("project.toml"), layer(project_body))),
    }
}

#[test]
fn source_names_and_display_are_stable() {
    assert_eq!(LayerName::User.name(), "user");
    assert_eq!(LayerName::Project.name(), "project");
    assert_eq!(Source::Default.to_string(), "default");
    assert_eq!(user().to_string(), "user file user.toml");
    assert_eq!(Source::Flag.to_string(), "--set flag");
}

#[test]
fn resolve_takes_each_key_from_the_first_layer_that_sets_it() {
    let registry = registry();
    let flags = parse_flags(&registry, &["tone=detailed".to_owned()]).unwrap();
    let resolved = resolve(
        &registry,
        &layers(
            "tone = \"brief\"\nlanguage = \"fr\"\nsearch.k = 12\n",
            "tone = \"normal\"\nlanguage = \"es\"\n",
        ),
        &flags,
    );
    let tone = resolved.get("tone").unwrap();
    assert_eq!(tone.value, Value::Text("detailed".to_owned()));
    assert_eq!(tone.source, Source::Flag);
    assert_eq!(
        tone.overridden,
        vec![
            (project(), Value::Text("normal".to_owned())),
            (user(), Value::Text("brief".to_owned())),
        ]
    );
    let language = resolved.get("language").unwrap();
    assert_eq!(language.value, Value::Text("es".to_owned()));
    assert_eq!(language.source, project());
    assert_eq!(
        language.overridden,
        vec![(user(), Value::Text("fr".to_owned()))]
    );
    let passages = resolved.get("search.k").unwrap();
    assert_eq!(passages.value, Value::Integer(12));
    assert_eq!(passages.source, user());
    let depth = resolved.get("search.rerank.depth").unwrap();
    assert_eq!(depth.value, Value::Integer(30));
    assert_eq!(depth.source, Source::Default);
    assert!(depth.overridden.is_empty());
    assert_eq!(resolved.iter().count(), registry.descriptors().len());
    assert_eq!(depth.descriptor.key, "search.rerank.depth");
}

#[test]
fn resolve_without_files_or_flags_gives_every_default() {
    let registry = registry();
    let resolved = resolve(&registry, &Layers::default(), &[]);
    for setting in resolved.iter() {
        assert_eq!(
            setting.source,
            Source::Default,
            "{}",
            setting.descriptor.key
        );
        assert_eq!(
            Some(&setting.value),
            registry.default_of(&setting.descriptor.key)
        );
    }
}

#[test]
fn resolved_values_read_back_as_their_types() {
    let registry = registry();
    let resolved = resolve(
        &registry,
        &layers(
            "search.routes.dense = false\nsearch.weights.dense = 2\n\
             search.section_prior.classes = [\"conversion\"]\n",
            "",
        ),
        &[],
    );
    assert_eq!(resolved.flag("search.routes.dense"), Some(false));
    assert_eq!(resolved.integer("search.k"), Some(10));
    assert_eq!(resolved.number("search.weights.dense"), Some(2.0));
    assert_eq!(resolved.text("tone"), Some("normal"));
    assert_eq!(
        resolved.list("search.section_prior.classes"),
        Some(&["conversion".to_owned()][..])
    );
    assert!(resolved.is_off("search.rerank.blend"));
    assert!(!resolved.is_off("search.k"));
    assert_eq!(resolved.flag("search.k"), None);
    assert_eq!(resolved.integer("tone"), None);
    assert_eq!(resolved.number("tone"), None);
    assert_eq!(resolved.text("search.k"), None);
    assert_eq!(resolved.list("tone"), None);
    assert_eq!(resolved.value("nothing"), None);
}

#[test]
fn parse_flags_refuses_unknown_repeated_malformed_and_locked_settings() {
    let registry = registry();
    let refusal = |texts: &[&str]| {
        let texts: Vec<String> = texts.iter().map(|text| (*text).to_owned()).collect();
        parse_flags(&registry, &texts).unwrap_err().to_string()
    };
    assert_eq!(refusal(&["tone"]), "--set tone: expected KEY=VALUE");
    assert_eq!(
        refusal(&["nothing=1"]),
        "--set nothing=1: unknown key \"nothing\""
    );
    assert_eq!(
        refusal(&["search.k=99"]),
        "--set search.k=99: expected a whole number from 1 to 50"
    );
    assert_eq!(
        refusal(&["tone=brief", "tone=normal"]),
        "--set tone=normal: tone is set twice"
    );
    let locked = [SettingDescriptor {
        key: Cow::Borrowed("fallback"),
        kind: SettingKind::Flag,
        default: Cow::Borrowed("false"),
        description: Cow::Borrowed("Locked."),
        class: SettingClass::Locked,
        standard_only: false,
    }];
    let locked = Registry::new(&locked).unwrap();
    assert_eq!(
        parse_flags(&locked, &["fallback=true".to_owned()])
            .unwrap_err()
            .to_string(),
        "--set fallback=true: the setting is locked: no file or flag may change it"
    );
    let flags = parse_flags(
        &registry,
        &["language=FR".to_owned(), "search.k=7".to_owned()],
    )
    .unwrap();
    assert_eq!(flags.len(), 2);
    assert_eq!(flags[0].key, "language");
    assert_eq!(flags[0].value, Value::Text("fr".to_owned()));
    assert_eq!(flags[1].key, "search.k");
}
