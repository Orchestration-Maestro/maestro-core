//! Values: each kind read from the command line's text and from a file's
//! TOML, refused with what it expects, and written back as both.

use crate::{ReservedValue, SettingKind, Value};
use std::borrow::Cow;
use toml::de::DeTable;

/// A choice of `values`.
fn choice(values: &'static [&'static str]) -> SettingKind {
    SettingKind::Choice {
        ordered: false,
        values: values.iter().map(|value| Cow::Borrowed(*value)).collect(),
        reserved: Cow::Borrowed(&[]),
    }
}

/// A list of `values`.
fn list(values: &'static [&'static str]) -> SettingKind {
    SettingKind::ChoiceList {
        values: values.iter().map(|value| Cow::Borrowed(*value)).collect(),
    }
}

/// The value `kind` reads from the TOML value `literal`.
fn from_toml(kind: &SettingKind, literal: &str) -> Result<Value, String> {
    let text = format!("value = {literal}\n");
    let table = DeTable::parse(&text).unwrap();
    let value = table.get_ref().get("value").unwrap();
    kind.parse_toml(value.get_ref())
        .map_err(|error| error.to_string())
}

/// `kind` read from `text`, refused with the reason rendered.
fn from_text(kind: &SettingKind, text: &str) -> Result<Value, String> {
    kind.parse_text(text).map_err(|error| error.to_string())
}

#[test]
fn parse_text_reads_whole_numbers_and_names_what_it_expects() {
    let integer = SettingKind::Integer {
        min: 1,
        max: 50,
        off: false,
    };
    assert_eq!(from_text(&integer, "50"), Ok(Value::Integer(50)));
    assert_eq!(from_text(&integer, "1"), Ok(Value::Integer(1)));
    assert_eq!(
        from_text(&integer, "51"),
        Err("expected a whole number from 1 to 50".to_owned())
    );
    assert_eq!(
        from_text(&integer, "0"),
        Err("expected a whole number from 1 to 50".to_owned())
    );
    assert_eq!(
        from_text(&integer, "off"),
        Err("expected a whole number from 1 to 50".to_owned())
    );
    let optional = SettingKind::Integer {
        min: 1,
        max: 30_000,
        off: true,
    };
    assert_eq!(from_text(&optional, "off"), Ok(Value::Off));
    assert_eq!(
        from_text(&optional, "x"),
        Err("expected a whole number from 1 to 30000, or \"off\"".to_owned())
    );
}

#[test]
fn parse_text_reads_numbers_and_flags() {
    let number = SettingKind::Number {
        min: 0.0,
        max: 1.0,
        off: true,
    };
    assert_eq!(from_text(&number, "0.6"), Ok(Value::Number(0.6)));
    assert_eq!(from_text(&number, "1"), Ok(Value::Number(1.0)));
    assert_eq!(from_text(&number, "off"), Ok(Value::Off));
    for refused in ["1.5", "-0.1", "NaN", "inf", ""] {
        assert_eq!(
            from_text(&number, refused),
            Err("expected a number from 0 to 1, or \"off\"".to_owned()),
            "{refused:?}"
        );
    }
    assert_eq!(from_text(&SettingKind::Flag, "true"), Ok(Value::Flag(true)));
    assert_eq!(
        from_text(&SettingKind::Flag, "false"),
        Ok(Value::Flag(false))
    );
    assert_eq!(
        from_text(&SettingKind::Flag, "yes"),
        Err("expected true or false".to_owned())
    );
}

#[test]
fn parse_text_reads_choices_languages_and_names() {
    let tone = choice(&["brief", "normal", "detailed"]);
    assert_eq!(
        from_text(&tone, "brief"),
        Ok(Value::Text("brief".to_owned()))
    );
    assert_eq!(
        from_text(&tone, "Brief"),
        Err("expected one of brief, normal, detailed".to_owned())
    );
    let classes = list(&["changelog", "release_notes", "conversion"]);
    assert_eq!(
        from_text(&classes, "conversion, changelog"),
        Ok(Value::List(vec![
            "conversion".to_owned(),
            "changelog".to_owned()
        ]))
    );
    assert_eq!(from_text(&classes, ""), Ok(Value::List(Vec::new())));
    for refused in ["changelog,changelog", "news", "changelog,"] {
        assert_eq!(
            from_text(&classes, refused),
            Err(
                "expected some of changelog, release_notes, conversion, each at most once"
                    .to_owned()
            ),
            "{refused:?}"
        );
    }
    let language = SettingKind::Language;
    assert_eq!(
        from_text(&language, "auto"),
        Ok(Value::Text("auto".to_owned()))
    );
    assert_eq!(
        from_text(&language, "ZH-hant-tw"),
        Ok(Value::Text("zh-Hant-TW".to_owned()))
    );
    assert_eq!(
        from_text(&language, "de-DE-1996"),
        Err(
            "expected auto or a language tag: only a language, a script and a region \
             are supported: variants, extensions and private use are not"
                .to_owned()
        )
    );
    let name = SettingKind::Name;
    assert_eq!(
        from_text(&name, "qwen3-4b"),
        Ok(Value::Text("qwen3-4b".to_owned()))
    );
    for refused in ["", ".", "..", "a/b", "é", &"a".repeat(65)] {
        assert_eq!(
            from_text(&name, refused),
            Err(
                "expected a router entry: 1 to 64 ASCII letters, digits, '.', '_' and '-', \
                 never '.' or '..' alone"
                    .to_owned()
            ),
            "{refused:?}"
        );
    }
    assert_eq!(
        from_text(&name, &"a".repeat(64)),
        Ok(Value::Text("a".repeat(64)))
    );
}

#[test]
fn a_reserved_choice_value_is_refused_with_its_reason_wherever_it_is_read() {
    let compute = SettingKind::Choice {
        ordered: false,
        values: Cow::Borrowed(&[Cow::Borrowed("off"), Cow::Borrowed("gpu")]),
        reserved: Cow::Borrowed(&[ReservedValue {
            value: Cow::Borrowed("cpu"),
            reason: Cow::Borrowed("cpu mode comes after M1"),
        }]),
    };
    assert_eq!(
        from_text(&compute, "off"),
        Ok(Value::Text("off".to_owned()))
    );
    assert_eq!(
        from_text(&compute, "cpu"),
        Err("cpu mode comes after M1".to_owned())
    );
    assert_eq!(
        from_toml(&compute, "\"cpu\""),
        Err("cpu mode comes after M1".to_owned())
    );
    assert_eq!(
        from_text(&compute, "tpu"),
        Err("expected one of off, gpu".to_owned())
    );
}

#[test]
fn parse_toml_reads_numbers_from_their_toml_types_only() {
    let integer = SettingKind::Integer {
        min: 1,
        max: 120,
        off: true,
    };
    assert_eq!(from_toml(&integer, "30"), Ok(Value::Integer(30)));
    assert_eq!(from_toml(&integer, "0x1e"), Ok(Value::Integer(30)));
    assert_eq!(from_toml(&integer, "\"off\""), Ok(Value::Off));
    for refused in ["\"30\"", "30.0", "true", "121", "\"on\""] {
        assert_eq!(
            from_toml(&integer, refused),
            Err("expected a whole number from 1 to 120, or \"off\"".to_owned()),
            "{refused}"
        );
    }
    let number = SettingKind::Number {
        min: 0.0,
        max: 100.0,
        off: false,
    };
    assert_eq!(from_toml(&number, "2"), Ok(Value::Number(2.0)));
    assert_eq!(from_toml(&number, "0.25"), Ok(Value::Number(0.25)));
    for refused in ["\"2\"", "nan", "inf", "-1.0", "\"off\""] {
        assert_eq!(
            from_toml(&number, refused),
            Err("expected a number from 0 to 100".to_owned()),
            "{refused}"
        );
    }
}

#[test]
fn parse_toml_reads_flags_choices_lists_and_languages_from_their_types_only() {
    assert_eq!(
        from_toml(&SettingKind::Flag, "false"),
        Ok(Value::Flag(false))
    );
    assert_eq!(
        from_toml(&SettingKind::Flag, "\"false\""),
        Err("expected true or false".to_owned())
    );
    let tone = choice(&["brief", "normal"]);
    assert_eq!(
        from_toml(&tone, "'brief'"),
        Ok(Value::Text("brief".to_owned()))
    );
    assert_eq!(
        from_toml(&tone, "[\"brief\"]"),
        Err("expected one of brief, normal".to_owned())
    );
    let classes = list(&["changelog", "conversion"]);
    assert_eq!(
        from_toml(&classes, "[\"conversion\"]"),
        Ok(Value::List(vec!["conversion".to_owned()]))
    );
    assert_eq!(from_toml(&classes, "[]"), Ok(Value::List(Vec::new())));
    for refused in [
        "\"conversion\"",
        "[1]",
        "[\"a\"]",
        "[\"conversion\", \"conversion\"]",
    ] {
        assert_eq!(
            from_toml(&classes, refused),
            Err("expected some of changelog, conversion, each at most once".to_owned()),
            "{refused}"
        );
    }
    assert_eq!(
        from_toml(&SettingKind::Language, "\"FR\""),
        Ok(Value::Text("fr".to_owned()))
    );
}

#[test]
fn values_write_back_as_the_toml_and_text_they_are_read_from() {
    let cases = [
        (SettingKind::Flag, Value::Flag(true), "true", "true"),
        (
            SettingKind::Integer {
                min: 0,
                max: 10,
                off: true,
            },
            Value::Integer(7),
            "7",
            "7",
        ),
        (
            SettingKind::Integer {
                min: 0,
                max: 10,
                off: true,
            },
            Value::Off,
            "\"off\"",
            "off",
        ),
        (
            SettingKind::Number {
                min: 0.0,
                max: 10.0,
                off: false,
            },
            Value::Number(1.0),
            "1.0",
            "1",
        ),
        (
            SettingKind::Number {
                min: 0.0,
                max: 10.0,
                off: false,
            },
            Value::Number(0.6),
            "0.6",
            "0.6",
        ),
        (
            choice(&["a\"b\\c"]),
            Value::Text("a\"b\\c".to_owned()),
            "\"a\\\"b\\\\c\"",
            "a\"b\\c",
        ),
        (
            list(&["x", "y"]),
            Value::List(vec!["y".to_owned(), "x".to_owned()]),
            "[\"y\", \"x\"]",
            "y,x",
        ),
        (list(&["x"]), Value::List(Vec::new()), "[]", ""),
    ];
    for (kind, value, toml, text) in cases {
        assert_eq!(value.to_toml(), toml, "{value:?}");
        assert_eq!(value.to_string(), text, "{value:?}");
        assert_eq!(from_toml(&kind, toml), Ok(value.clone()), "{toml}");
        assert_eq!(from_text(&kind, text), Ok(value.clone()), "{text}");
    }
}

#[test]
fn to_toml_escapes_every_character_a_basic_string_cannot_hold() {
    assert_eq!(
        Value::Text("a\u{7}\tb\n\u{7f}é".to_owned()).to_toml(),
        "\"a\\u0007\\tb\\n\\u007Fé\""
    );
}

#[test]
fn values_render_as_json_for_machine_output() {
    assert_eq!(Value::Flag(false).to_json(), serde_json::json!(false));
    assert_eq!(Value::Integer(5).to_json(), serde_json::json!(5));
    assert_eq!(Value::Number(0.5).to_json(), serde_json::json!(0.5));
    assert_eq!(
        Value::Text("fr".to_owned()).to_json(),
        serde_json::json!("fr")
    );
    assert_eq!(
        Value::List(vec!["a".to_owned()]).to_json(),
        serde_json::json!(["a"])
    );
    assert_eq!(Value::Off.to_json(), serde_json::json!("off"));
}
