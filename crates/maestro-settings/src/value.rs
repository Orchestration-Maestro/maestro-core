//! A setting's value: read from the command line's text or a file's TOML
//! against its kind, and written back as either, or as JSON.

use crate::{
    descriptor::{SettingKind, Text},
    language::canonical_language,
};
use std::{error, fmt, fmt::Write as _};
use toml::de::DeValue;

/// The text a kind with `off` accepts for its "not set".
pub const OFF: &str = "off";

/// The text `language` takes for the question's language.
pub const AUTO: &str = "auto";

/// A value its kind accepted.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A flag's.
    Flag(bool),
    /// A whole number's.
    Integer(i64),
    /// A number's.
    Number(f64),
    /// A choice's, a name's, or a language's: `auto` or a canonical tag.
    Text(String),
    /// A choice list's, in the order given.
    List(Vec<String>),
    /// The "not set" of a kind with `off`.
    Off,
}

impl Value {
    /// The value as a TOML literal, as `config set` writes it in a file.
    #[must_use]
    pub fn to_toml(&self) -> String {
        match self {
            Self::Flag(flag) => flag.to_string(),
            Self::Integer(integer) => integer.to_string(),
            // Debug keeps the decimal point a TOML float needs: `1.0`.
            Self::Number(number) => format!("{number:?}"),
            Self::Text(text) => basic_string(text),
            Self::List(items) => {
                let items: Vec<String> = items.iter().map(|item| basic_string(item)).collect();
                format!("[{}]", items.join(", "))
            }
            Self::Off => basic_string(OFF),
        }
    }

    /// The value as JSON, for `--json` output and the journal.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Flag(flag) => (*flag).into(),
            Self::Integer(integer) => (*integer).into(),
            Self::Number(number) => (*number).into(),
            Self::Text(text) => text.as_str().into(),
            Self::List(items) => items.as_slice().into(),
            Self::Off => OFF.into(),
        }
    }
}

impl fmt::Display for Value {
    /// The value as the command line writes it, which
    /// [`SettingKind::parse_text`] reads back.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flag(flag) => write!(formatter, "{flag}"),
            Self::Integer(integer) => write!(formatter, "{integer}"),
            Self::Number(number) => write!(formatter, "{number}"),
            Self::Text(text) => formatter.write_str(text),
            Self::List(items) => formatter.write_str(&items.join(",")),
            Self::Off => formatter.write_str(OFF),
        }
    }
}

/// `text` as a TOML basic string: quoted, with a quote, a backslash and every
/// control character but the tab escaped.
fn basic_string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for character in text.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\t' => quoted.push_str("\\t"),
            '\n' => quoted.push_str("\\n"),
            control if control.is_control() => {
                // Writing to a String cannot fail.
                let _written = write!(quoted, "\\u{:04X}", u32::from(control));
            }
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

/// Why a value was refused: what its kind expects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueError(String);

impl fmt::Display for ValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl error::Error for ValueError {}

impl SettingKind {
    /// The value `text` gives, as the command line writes it: `true`, `30`,
    /// `0.6`, `brief`, `changelog,conversion` or `off`.
    ///
    /// # Errors
    ///
    /// [`ValueError`], saying what the kind expects, for any other text.
    pub fn parse_text(&self, text: &str) -> Result<Value, ValueError> {
        let accepted = match self {
            Self::Integer { off: true, .. } | Self::Number { off: true, .. } if text == OFF => {
                Some(Value::Off)
            }
            Self::Flag => text.parse().ok().map(Value::Flag),
            Self::Integer { .. } => text.parse().ok().and_then(|integer| self.integer(integer)),
            Self::Number { .. } => text.parse().ok().and_then(|number| self.number(number)),
            Self::Choice { values, reserved } => {
                if let Some(reserved) = reserved.iter().find(|reserved| reserved.value == text) {
                    return Err(ValueError(reserved.reason.to_string()));
                }
                values
                    .iter()
                    .any(|value| value == text)
                    .then(|| Value::Text(text.to_owned()))
            }
            Self::ChoiceList { values } => {
                let items = if text.is_empty() {
                    Vec::new()
                } else {
                    text.split(',').map(str::trim).collect()
                };
                list(values, items)
            }
            Self::Language => return language(text),
            Self::Name => is_name(text).then(|| Value::Text(text.to_owned())),
        };
        accepted.ok_or_else(|| self.refusal())
    }

    /// The value the TOML value `value` gives: a flag from a boolean, a whole
    /// number from an integer, a number from a float or an integer, a list
    /// from an array of strings, and the rest, `"off"` included, from a
    /// string.
    ///
    /// # Errors
    ///
    /// [`ValueError`], saying what the kind expects, for a value of another
    /// TOML type or out of range.
    pub fn parse_toml(&self, value: &DeValue<'_>) -> Result<Value, ValueError> {
        let accepted = match (self, value) {
            (Self::Flag, DeValue::Boolean(flag)) => Some(Value::Flag(*flag)),
            (Self::Integer { .. }, DeValue::Integer(integer)) => {
                i64::from_str_radix(integer.as_str(), integer.radix())
                    .ok()
                    .and_then(|integer| self.integer(integer))
            }
            (Self::Number { .. }, DeValue::Integer(integer)) => {
                i64::from_str_radix(integer.as_str(), integer.radix())
                    .ok()
                    .and_then(|integer| self.whole_number(integer))
            }
            (Self::Number { .. }, DeValue::Float(float)) => float
                .as_str()
                .parse()
                .ok()
                .and_then(|number| self.number(number)),
            (Self::ChoiceList { values }, DeValue::Array(items)) => {
                let texts: Option<Vec<&str>> = items
                    .iter()
                    .map(|item| match item.get_ref() {
                        DeValue::String(text) => Some(text.as_ref()),
                        _ => None,
                    })
                    .collect();
                texts.and_then(|texts| list(values, texts))
            }
            (
                Self::Integer { off: true, .. } | Self::Number { off: true, .. },
                DeValue::String(text),
            ) if text == OFF => Some(Value::Off),
            (Self::Choice { .. } | Self::Language | Self::Name, DeValue::String(text)) => {
                return self.parse_text(text);
            }
            _ => None,
        };
        accepted.ok_or_else(|| self.refusal())
    }

    /// `integer` when it is in range.
    fn integer(&self, integer: i64) -> Option<Value> {
        match self {
            Self::Integer { min, max, .. } => (*min..=*max)
                .contains(&integer)
                .then_some(Value::Integer(integer)),
            _ => None,
        }
    }

    /// The whole number `integer` as a number, when it is in range: a file
    /// may write `1` for `1.0`.
    fn whole_number(&self, integer: i64) -> Option<Value> {
        // A setting's numbers are small: the conversion is exact for them.
        #[expect(
            clippy::cast_precision_loss,
            reason = "numbers beyond 2^53 are out of every setting's range"
        )]
        let number = integer as f64;
        self.number(number)
    }

    /// `number` when it is finite and in range.
    fn number(&self, number: f64) -> Option<Value> {
        match self {
            Self::Number { min, max, .. } => (number.is_finite()
                && (*min..=*max).contains(&number))
            .then_some(Value::Number(number)),
            _ => None,
        }
    }

    /// The refusal of a value the kind does not accept.
    fn refusal(&self) -> ValueError {
        ValueError(format!("expected {}", self.expectation()))
    }

    /// What the kind accepts, in words: "a whole number from 1 to 50".
    #[must_use]
    pub fn expectation(&self) -> String {
        let or_off = |off: bool| if off { ", or \"off\"" } else { "" };
        match self {
            Self::Flag => "true or false".to_owned(),
            Self::Integer { min, max, off } => {
                format!("a whole number from {min} to {max}{}", or_off(*off))
            }
            Self::Number { min, max, off } => {
                format!("a number from {min} to {max}{}", or_off(*off))
            }
            Self::Choice { values, .. } => format!("one of {}", values.join(", ")),
            Self::ChoiceList { values } => {
                format!("some of {}, each at most once", values.join(", "))
            }
            Self::Language => format!("{AUTO} or a language tag such as en, fr or es-419"),
            Self::Name => "a router entry: 1 to 64 ASCII letters, digits, '.', '_' and '-', \
                           never '.' or '..' alone"
                .to_owned(),
        }
    }
}

/// `items` as a list when each is one of `values`, once.
fn list(values: &[Text], items: Vec<&str>) -> Option<Value> {
    let mut accepted: Vec<String> = Vec::with_capacity(items.len());
    for item in items {
        if !values.iter().any(|value| value == item) || accepted.iter().any(|seen| seen == item) {
            return None;
        }
        accepted.push(item.to_owned());
    }
    Some(Value::List(accepted))
}

/// `auto`, or the canonical form of the language tag `text`.
fn language(text: &str) -> Result<Value, ValueError> {
    if text == AUTO {
        return Ok(Value::Text(AUTO.to_owned()));
    }
    canonical_language(text)
        .map(Value::Text)
        .map_err(|reason| ValueError(format!("expected {AUTO} or a language tag: {reason}")))
}

/// Whether `text` is a router entry's name: 1 to 64 ASCII letters, digits,
/// `.`, `_` and `-`, never `.` or `..` alone.
fn is_name(text: &str) -> bool {
    (1..=64).contains(&text.len())
        && !matches!(text, "." | "..")
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}
