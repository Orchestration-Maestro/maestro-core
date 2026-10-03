//! Const descriptor construction shared by the built-in declarations.

use crate::descriptor::{SettingClass, SettingDescriptor, SettingKind, Texts};
use std::borrow::Cow;

/// The choice values `$value`, as a descriptor's const texts.
macro_rules! texts {
    ($($value:literal),+ $(,)?) => {
        Cow::Borrowed(&[$(Cow::Borrowed($value)),+])
    };
}

/// A setting descriptor with its declared override class.
pub(super) const fn setting(
    key: &'static str,
    kind: SettingKind,
    default: &'static str,
    description: &'static str,
    class: SettingClass,
) -> SettingDescriptor {
    SettingDescriptor {
        key: Cow::Borrowed(key),
        kind,
        default: Cow::Borrowed(default),
        description: Cow::Borrowed(description),
        class,
        standard_only: false,
    }
}

/// Declares one descriptor with the Free override class.
macro_rules! free {
    ($key:expr, $kind:expr, $default:expr, $description:expr $(,)?) => {
        setting($key, $kind, $default, $description, SettingClass::Free)
    };
}

/// Declares one descriptor with the Bounded override class.
macro_rules! bounded {
    ($key:expr, $kind:expr, $default:expr, $description:expr $(,)?) => {
        setting($key, $kind, $default, $description, SettingClass::Bounded)
    };
}

/// Declares one descriptor with the Locked override class.
macro_rules! locked {
    ($key:expr, $kind:expr, $default:expr, $description:expr $(,)?) => {
        setting($key, $kind, $default, $description, SettingClass::Locked)
    };
}

pub(super) use {bounded, free, locked, texts};

/// A whole number from `min` to `max`.
pub(super) const fn integer(min: i64, max: i64) -> SettingKind {
    SettingKind::Integer {
        min,
        max,
        off: false,
        power_of_two: false,
    }
}

/// A whole number from `min` to `max`, or `off`.
pub(super) const fn optional_integer(min: i64, max: i64) -> SettingKind {
    SettingKind::Integer {
        min,
        max,
        off: true,
        power_of_two: false,
    }
}

/// A number from `min` to `max`, `off` too when `off`.
pub(super) const fn number(min: f64, max: f64, off: bool) -> SettingKind {
    SettingKind::Number { min, max, off }
}

/// One of `values`.
pub(super) const fn choice(values: Texts) -> SettingKind {
    ordered_choice(values, false)
}

/// One of `values`, whose declaration also provides its restriction order.
pub(super) const fn ordered_choice(values: Texts, ordered: bool) -> SettingKind {
    SettingKind::Choice {
        ordered,
        values,
        reserved: Cow::Borrowed(&[]),
    }
}

/// Each route's weight in reciprocal rank fusion: finite and nonnegative,
/// as search requires, bounded for a hand-edited file.
pub(super) const WEIGHT: SettingKind = number(0.0, 100.0, false);
