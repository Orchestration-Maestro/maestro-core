//! A preferences file parsed strictly: its schema marker required, every key
//! and type checked, each refusal naming the key.

use crate::{Layer, MAX_FILE_BYTES, Registry, SettingClass, SettingDescriptor, SettingKind, Value};
use std::borrow::Cow;

/// The built-in registry.
fn registry() -> Registry {
    Registry::built_in().unwrap()
}

/// Why `text` is refused as a preferences file, rendered.
fn refusal(text: &str) -> String {
    Layer::parse(&registry(), text).unwrap_err().to_string()
}

#[test]
fn parse_reads_every_form_toml_writes_a_setting_in() {
    let layer = Layer::parse(
        &registry(),
        "# Mine\nschema = \"maestro-preferences/1\"\nlanguage = \"FR\"\ntone = 'brief'\n\
         ask.k = 8\n\n[search]\nk = 20\nweights = { dense = 2, lexical = 0.5 }\n\
         rerank.blend = \"off\"\n\n[search.section_prior]\nclasses = [\n  \"conversion\",\n]\n\
         weight = 0.6\n",
    )
    .unwrap();
    let expected = [
        ("ask.k", Value::Integer(8)),
        ("language", Value::Text("fr".to_owned())),
        ("search.k", Value::Integer(20)),
        ("search.rerank.blend", Value::Off),
        (
            "search.section_prior.classes",
            Value::List(vec!["conversion".to_owned()]),
        ),
        ("search.section_prior.weight", Value::Number(0.6)),
        ("search.weights.dense", Value::Number(2.0)),
        ("search.weights.lexical", Value::Number(0.5)),
        ("tone", Value::Text("brief".to_owned())),
    ];
    let read: Vec<(&str, &Value)> = layer.iter().collect();
    let expected: Vec<(&str, &Value)> = expected.iter().map(|(key, value)| (*key, value)).collect();
    assert_eq!(read, expected);
    assert_eq!(layer.get("tone"), Some(&Value::Text("brief".to_owned())));
    assert_eq!(layer.get("search.rrf_k"), None);
}

#[test]
fn parse_reads_a_file_of_the_schema_alone_as_an_empty_layer() {
    let layer = Layer::parse(
        &registry(),
        "schema = \"maestro-preferences/1\"\n[search]\n",
    )
    .unwrap();
    assert!(layer.is_empty());
}

#[test]
fn parse_refuses_a_missing_or_other_schema() {
    assert_eq!(
        refusal(""),
        "the file has no schema = \"maestro-preferences/1\" line"
    );
    assert_eq!(
        refusal("tone = \"brief\"\n"),
        "the file has no schema = \"maestro-preferences/1\" line"
    );
    assert_eq!(
        refusal("schema = \"maestro-preferences/2\"\n"),
        "schema: expected \"maestro-preferences/1\", found \"maestro-preferences/2\""
    );
    assert_eq!(
        refusal("schema = 1\n"),
        "schema: expected \"maestro-preferences/1\", found 1"
    );
}

#[test]
fn parse_names_each_unknown_key_with_its_table() {
    let schema = "schema = \"maestro-preferences/1\"\n";
    for (body, key) in [
        ("updates = \"off\"\n", "updates"),
        ("[access]\nread = []\n", "access"),
        ("[search]\nfoo = 1\n", "search.foo"),
        ("[search.rerank]\nfoo = 1\n", "search.rerank.foo"),
        ("search.rerank.foo.bar = 1\n", "search.rerank.foo"),
        ("[overrides]\nmodel_profile = \"fast\"\n", "overrides"),
    ] {
        assert_eq!(
            refusal(&format!("{schema}{body}")),
            format!("unknown key {key:?}"),
            "{body}"
        );
    }
}

#[test]
fn parse_refuses_a_value_of_another_type_naming_the_key_and_what_it_found() {
    let schema = "schema = \"maestro-preferences/1\"\n";
    for (body, message) in [
        (
            "tone = \"loud\"\n",
            "tone: expected one of brief, normal, detailed, found \"loud\"",
        ),
        (
            "[search]\nk = \"20\"\n",
            "search.k: expected a whole number from 1 to 50, found \"20\"",
        ),
        (
            "[search]\nk = 51\n",
            "search.k: expected a whole number from 1 to 50, found 51",
        ),
        ("search = 5\n", "search: expected a table, found 5"),
        (
            "[[search]]\nk = 5\n",
            "search: expected a table, found [[search]]",
        ),
        (
            "[search.k]\nx = 1\n",
            "search.k: expected a whole number from 1 to 50, found [search.k]",
        ),
        (
            "language = \"en-US-x-a\"\n",
            "language: expected auto or a language tag: only a language, a script and a region \
             are supported: variants, extensions and private use are not, found \"en-US-x-a\"",
        ),
    ] {
        assert_eq!(refusal(&format!("{schema}{body}")), message, "{body}");
    }
}

#[test]
fn parse_refuses_text_that_is_not_toml_or_is_too_large() {
    let duplicated =
        refusal("schema = \"maestro-preferences/1\"\ntone = \"brief\"\ntone = \"normal\"\n");
    assert!(
        duplicated.starts_with("the file is not TOML: "),
        "{duplicated}"
    );
    let broken = refusal("schema = \n");
    assert!(broken.starts_with("the file is not TOML: "), "{broken}");
    let padding = " ".repeat(MAX_FILE_BYTES);
    assert_eq!(
        refusal(&format!("schema = \"maestro-preferences/1\"\n{padding}")),
        format!(
            "the file holds {} bytes, more than the {MAX_FILE_BYTES} a preferences file may",
            MAX_FILE_BYTES + 33
        )
    );
    let at_the_limit = format!(
        "schema = \"maestro-preferences/1\"\n{}",
        " ".repeat(MAX_FILE_BYTES - 33)
    );
    assert!(Layer::parse(&registry(), &at_the_limit).is_ok());
}

#[test]
fn parse_refuses_a_locked_setting_in_any_file() {
    let descriptors = [SettingDescriptor {
        key: Cow::Borrowed("raw_prompt_logging"),
        kind: SettingKind::Flag,
        default: Cow::Borrowed("false"),
        description: Cow::Borrowed("Locked."),
        class: SettingClass::Locked,
    }];
    let registry = Registry::new(&descriptors).unwrap();
    assert_eq!(
        Layer::parse(
            &registry,
            "schema = \"maestro-preferences/1\"\nraw_prompt_logging = false\n"
        )
        .unwrap_err()
        .to_string(),
        "raw_prompt_logging: the setting is locked: no file or flag may change it"
    );
}

#[test]
fn parse_refuses_a_setting_a_quoted_dotted_key_and_a_table_both_set() {
    let quoted_then_table =
        "schema = \"maestro-preferences/1\"\n\"search.k\" = 3\n[search]\nk = 9\n";
    assert_eq!(
        refusal(quoted_then_table),
        "search.k: the setting is set twice in the file"
    );
    let table_then_quoted = "schema = \"maestro-preferences/1\"\n\"ask.k\" = 3\n[ask]\nk = 9\n";
    assert_eq!(
        refusal(table_then_quoted),
        "ask.k: the setting is set twice in the file"
    );
}
