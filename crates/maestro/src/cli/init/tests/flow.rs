//! Registry injection and renderer-independent draft/navigation regression tests.
use super::super::{
    flow::{self, Answer, Draft, FlowPort},
    plain::Plain,
};
use maestro_settings::{LayerName, Layers, Registry, SettingClass, SettingKind};

fn registry() -> Registry {
    let base = Registry::built_in().unwrap();
    let mut descriptors: Vec<_> = base.descriptors().cloned().collect();
    let mut added = descriptors[0].clone();
    added.key = "synthetic_setting".into();
    added.kind = SettingKind::Integer {
        min: 0,
        max: 9,
        power_of_two: false,
        off: false,
    };
    added.default = "3".into();
    added.description = "Synthetic descriptor, no screen registration.".into();
    added.class = SettingClass::Free;
    descriptors.push(added.clone());
    added.key = "synthetic_standard".into();
    added.standard_only = true;
    descriptors.push(added);
    Registry::new(&descriptors).unwrap()
}

#[test]
fn catalog_init_flow_injected_descriptor_appears_and_edits_in_both_entry_modes() {
    for (init, layer) in [(true, LayerName::Project), (false, LayerName::User)] {
        let mut draft = Draft::new(registry(), Layers::default(), layer, &[], init).unwrap();
        let mut output = Vec::new();
        let mut input = "synthetic_setting=8\n\n".as_bytes();
        let mut port = Plain {
            input: &mut input,
            output: &mut output,
        };
        assert_eq!(
            flow::editor(&mut port, &mut draft).unwrap(),
            Answer::Text(String::new())
        );
        assert!(draft.choices.contains(&"synthetic_setting=8".to_owned()));
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("synthetic_setting = 3"), "{text}");
        assert!(text.contains("synthetic_setting = 8"), "{text}");
        assert!(text.contains("Synthetic descriptor, no screen registration."));
        assert!(text.contains("source: default"));
        assert!(
            text.contains("accepts: a whole number from 0 to 9"),
            "{text}"
        );
        assert!(text.contains("Standard-only:"));
        assert!(text.contains("Locked:"));
    }
}

#[test]
fn catalog_init_flow_refusals_retain_draft_and_allowed_neighbours_work() {
    let mut draft =
        Draft::new(registry(), Layers::default(), LayerName::Project, &[], true).unwrap();
    draft.edit("synthetic_setting=8").unwrap();
    for text in [
        "synthetic_setting=10",
        "synthetic_setting=-1",
        "raw_prompt_logging=true",
        "synthetic_standard=4",
        "trust=true",
        "language=auto",
        "updates=auto",
        "invalid",
    ] {
        let before = draft.choices.clone();
        assert!(draft.edit(text).is_err(), "{text}");
        assert_eq!(draft.choices, before, "{text}");
    }
    for text in [
        "synthetic_setting=0",
        "synthetic_setting=9",
        "updates=off",
        "language=zh-hant-tw",
        "tone=detailed",
    ] {
        draft.edit(text).unwrap();
    }
    assert!(draft.choices.contains(&"language=zh-Hant-TW".to_owned()));
    let mut user = Draft::new(registry(), Layers::default(), LayerName::User, &[], false).unwrap();
    user.edit("updates=auto").unwrap();
    assert_eq!(user.choices, ["updates=auto"]);
}

#[test]
fn catalog_init_flow_errors_keep_focus_and_back_preserves_choices() {
    let mut draft =
        Draft::new(registry(), Layers::default(), LayerName::Project, &[], true).unwrap();
    let mut output = Vec::new();
    let mut input = "not-a-tag-9999\nzh-Hant-TW\nback\n".as_bytes();
    let mut port = Plain {
        input: &mut input,
        output: &mut output,
    };
    assert_eq!(
        flow::preference(&mut port, &mut draft, "language", "English artifacts").unwrap(),
        Answer::Text("zh-Hant-TW".to_owned())
    );
    assert_eq!(flow::editor(&mut port, &mut draft).unwrap(), Answer::Back);
    assert_eq!(draft.choices, ["language=zh-Hant-TW"]);
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Error:"), "{text}");
    assert_eq!(text.matches("language [Enter keeps current]").count(), 2);
}

#[test]
fn catalog_init_flow_plain_eof_control_c_escape_and_default_no() {
    for (text, answer) in [
        ("", Answer::Cancel),
        ("\u{3}\n", Answer::Cancel),
        ("cancel\n", Answer::Cancel),
        ("\u{1b}\n", Answer::Back),
        ("back\n", Answer::Back),
        ("\n", Answer::Text(String::new())),
    ] {
        let mut input = text.as_bytes();
        let mut output = Vec::new();
        let mut plain = Plain {
            input: &mut input,
            output: &mut output,
        };
        assert_eq!(plain.ask("Label: ").unwrap(), answer);
        assert_eq!(output, b"Label: ");
    }
}
