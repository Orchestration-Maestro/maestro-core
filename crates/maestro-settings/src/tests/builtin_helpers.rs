//! Runtime checks for descriptors otherwise constructed only in const initializers.

use crate::{
    SettingClass, SettingDescriptor, SettingKind, Texts,
    builtin_helpers::{
        WEIGHT, bounded, choice, free, integer, locked, number, optional_integer, ordered_choice,
        setting, texts,
    },
};
use std::{borrow::Cow, hint::black_box};

#[test]
fn builtin_helpers_preserve_descriptor_fields_and_override_classes() {
    for class in [
        SettingClass::Free,
        SettingClass::Bounded,
        SettingClass::Locked,
    ] {
        let descriptor = setting(
            black_box("synthetic.limit"),
            black_box(SettingKind::Flag),
            black_box("true"),
            black_box("Synthetic description"),
            black_box(class),
        );
        assert_eq!(
            descriptor,
            SettingDescriptor {
                key: Cow::Borrowed("synthetic.limit"),
                kind: SettingKind::Flag,
                default: Cow::Borrowed("true"),
                description: Cow::Borrowed("Synthetic description"),
                class,
                standard_only: false,
            }
        );
    }
    let descriptors = [
        free!(
            black_box("synthetic.free"),
            SettingKind::Flag,
            "false",
            "Free text"
        ),
        bounded!(
            black_box("synthetic.bounded"),
            integer(1, 8),
            "4",
            "Bounded text"
        ),
        locked!(
            black_box("synthetic.locked"),
            SettingKind::Name,
            "name",
            "Locked text"
        ),
    ];
    for (descriptor, class, key, kind, default, description) in [
        (
            &descriptors[0],
            SettingClass::Free,
            "synthetic.free",
            SettingKind::Flag,
            "false",
            "Free text",
        ),
        (
            &descriptors[1],
            SettingClass::Bounded,
            "synthetic.bounded",
            integer(1, 8),
            "4",
            "Bounded text",
        ),
        (
            &descriptors[2],
            SettingClass::Locked,
            "synthetic.locked",
            SettingKind::Name,
            "name",
            "Locked text",
        ),
    ] {
        assert_eq!(descriptor.class, class);
        assert_eq!(descriptor.key, key);
        assert_eq!(descriptor.kind, kind);
        assert_eq!(descriptor.default, default);
        assert_eq!(descriptor.description, description);
        assert!(!descriptor.standard_only);
    }
}

#[test]
fn builtin_helpers_preserve_bounds_off_and_power_of_two() {
    assert_eq!(
        integer(black_box(-2), black_box(17)),
        SettingKind::Integer {
            min: -2,
            max: 17,
            off: false,
            power_of_two: false,
        }
    );
    assert_eq!(
        optional_integer(black_box(0), black_box(23)),
        SettingKind::Integer {
            min: 0,
            max: 23,
            off: true,
            power_of_two: false,
        }
    );
    for off in [false, true] {
        assert_eq!(
            number(black_box(-0.5), black_box(3.5), black_box(off)),
            SettingKind::Number {
                min: -0.5,
                max: 3.5,
                off
            }
        );
    }
    assert_eq!(
        black_box(WEIGHT),
        SettingKind::Number {
            min: 0.0,
            max: 100.0,
            off: false
        }
    );
}

#[test]
fn builtin_helpers_preserve_choice_texts_order_and_empty_reservations() {
    let values: Texts = texts!["low", "high",];
    assert_eq!(
        values.as_ref(),
        [Cow::Borrowed("low"), Cow::Borrowed("high")]
    );
    for ordered in [false, true] {
        assert_eq!(
            ordered_choice(black_box(values.clone()), black_box(ordered)),
            SettingKind::Choice {
                values: values.clone(),
                ordered,
                reserved: Cow::Borrowed(&[])
            }
        );
    }
    assert_eq!(
        choice(black_box(values.clone())),
        SettingKind::Choice {
            values,
            ordered: false,
            reserved: Cow::Borrowed(&[])
        }
    );
}
