//! A setting's descriptor: everything Maestro knows about one setting, as
//! data. Adding a setting is adding one descriptor to the registry; parsing,
//! checking, resolving, editing, explaining and journaling it need no code of
//! their own.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// How a setting's layers combine: its override class (S3 FR-S3-014,
/// architecture 03 §1.6). Every setting has exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingClass {
    /// The first layer that sets it wins: an explicit flag, then the project
    /// file, then the user file, then the built-in default.
    Free,
    /// Layers may only narrow the user's or the default bound; resolved by
    /// S3's restrictive resolution (C17), which the registry refuses until
    /// then.
    Bounded,
    /// Every layer's values accumulate; resolved by S3's restrictive
    /// resolution (C17), which the registry refuses until then.
    Additive,
    /// No layer may change its default: a layer that sets it is refused.
    Locked,
}

impl SettingClass {
    /// Its name, as descriptors and `config explain` write it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Bounded => "bounded",
            Self::Additive => "additive",
            Self::Locked => "locked",
        }
    }
}

/// The values a setting accepts. A kind with `off` also accepts the text
/// `"off"`, the setting's "not set" (a threshold with none, a policy
/// switched off), which a file can write where TOML has no null.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SettingKind {
    /// `true` or `false`.
    Flag,
    /// A whole number from `min` to `max`.
    Integer {
        /// The least accepted.
        min: i64,
        /// The greatest accepted.
        max: i64,
        /// Whether positive powers of two are the only accepted integers.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        power_of_two: bool,
        /// Whether `"off"` is accepted too.
        off: bool,
    },
    /// A finite number from `min` to `max`; a file may write a whole one.
    Number {
        /// The least accepted.
        min: f64,
        /// The greatest accepted.
        max: f64,
        /// Whether `"off"` is accepted too.
        off: bool,
    },
    /// One of `values`.
    Choice {
        /// Whether the values declare a low-to-high restriction order.
        #[serde(default)]
        ordered: bool,
        /// The accepted values.
        values: Texts,
        /// Values named but not available yet, each refused with its reason.
        #[serde(default, skip_serializing_if = "<[_]>::is_empty")]
        reserved: Reserved,
    },
    /// Some of `values`, each at most once: an array in a file, and the
    /// values joined by commas on the command line.
    ChoiceList {
        /// The accepted values.
        values: Texts,
    },
    /// `auto`, the question's language, or a language tag of the BCP 47
    /// subset S3 ruled: a 2-3 letter language, an optional 4-letter script
    /// and an optional 2-letter or 3-digit region, stored in canonical case.
    Language,
    /// A name of 1 to 64 ASCII letters, digits, `.`, `_` and `-`, never `.`
    /// or `..` alone: a model router entry.
    Name,
}

/// A text of a descriptor: borrowed in the built-in consts, owned when read.
pub type Text = Cow<'static, str>;

/// The texts of a choice: borrowed in the built-in consts, owned when read.
pub type Texts = Cow<'static, [Text]>;

/// A choice's value named but not available yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReservedValue {
    /// The value.
    pub value: Text,
    /// Why it is refused, as the refusal says it: "cpu mode comes after M1".
    pub reason: Text,
}

/// A choice's reserved values: borrowed in the built-in consts, owned when
/// read.
pub type Reserved = Cow<'static, [ReservedValue]>;

/// One setting, as the registry declares it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingDescriptor {
    /// Its dotted key: lower-case segments of letters, digits and `_`, each
    /// starting with a letter, such as `search.rerank.depth`. In a file the
    /// segments before the last name its table.
    pub key: Text,
    /// The values it accepts.
    pub kind: SettingKind,
    /// Its built-in default, written as `config set` takes a value.
    pub default: Text,
    /// What it does, in one line.
    pub description: Text,
    /// How its layers combine.
    pub class: SettingClass,
    /// Only central standards may declare this setting in catalog sources.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub standard_only: bool,
}
