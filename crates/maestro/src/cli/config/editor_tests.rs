//! The shared editor's injected descriptor is persisted by S1's journaled API.
use super::{Change, Places, change::run_in_registry, editor::collect};
use crate::{
    cli::{
        init::{
            flow::{Draft, validate},
            plain::Plain,
        },
        output::Output,
    },
    kernel::Kernel,
};
use maestro_kernel::scope::LOCAL;
use maestro_settings::{LayerName, Layers, Registry, SettingClass, SettingKind};
use maestro_test_scratch::scratch_directory;
use std::fs;

#[test]
fn catalog_init_menu_config_injected_descriptor_uses_existing_api_and_journal() {
    let root = scratch_directory().unwrap();
    let places = Places {
        config_dir: root.join("config"),
        working: Some(root.join("home/work")),
        home: Some(root.join("home")),
    };
    fs::create_dir_all(places.working.as_ref().unwrap()).unwrap();
    fs::create_dir_all(&places.config_dir).unwrap();
    let built_in = Registry::built_in().unwrap();
    let mut descriptors: Vec<_> = built_in.descriptors().cloned().collect();
    let mut added = descriptors[0].clone();
    added.key = "injected_editor".into();
    added.kind = SettingKind::Flag;
    added.default = "false".into();
    added.description = "Injected every-setting editor entry.".into();
    added.class = SettingClass::Free;
    descriptors.push(added);
    let registry = Registry::new(&descriptors).unwrap();
    let mut draft = Draft::new(
        registry.clone(),
        Layers::default(),
        LayerName::User,
        &[],
        false,
    )
    .unwrap();
    let mut input = "injected_editor=true\n\nyes\n".as_bytes();
    let mut rendered = Vec::new();
    assert!(
        collect(
            &mut Plain {
                input: &mut input,
                output: &mut rendered
            },
            &mut draft
        )
        .unwrap()
    );
    assert!(
        String::from_utf8(rendered)
            .unwrap()
            .contains("injected_editor = false")
    );
    run_in_registry(
        Output::new(true),
        Change {
            key: "injected_editor",
            value: Some("true"),
            layer: draft.layer,
        },
        (&places, &registry),
        || Kernel::open_at(&root.join("data"), &places.config_dir),
        |kernel, record| {
            kernel
                .database
                .record_setting_change(record)
                .map(drop)
                .map_err(|error| error.to_string())
        },
    )
    .unwrap();
    assert!(
        fs::read_to_string(places.config_dir.join("preferences.toml"))
            .unwrap()
            .contains("injected_editor = true")
    );
    let kernel = Kernel::open_at(&root.join("data"), &places.config_dir).unwrap();
    let changes = kernel.database.setting_changes(LOCAL).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change.key, "injected_editor");
    assert_eq!(changes[0].change.new, Some(serde_json::json!(true)));
    drop(kernel);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn catalog_init_menu_config_descriptor_and_layer_restrictions_precede_effects() {
    let built_in = Registry::built_in().unwrap();
    let mut descriptors: Vec<_> = built_in.descriptors().cloned().collect();
    let mut central = descriptors[0].clone();
    central.key = "central_only".into();
    central.standard_only = true;
    descriptors.push(central);
    let registry = Registry::new(&descriptors).unwrap();
    for (key, value, layer) in [
        ("raw_prompt_logging", "true", LayerName::User),
        ("central_only", "fr", LayerName::User),
        ("updates", "auto", LayerName::Project),
        ("access.read", "true", LayerName::User),
    ] {
        let root = scratch_directory().unwrap();
        let places = Places {
            config_dir: root.join("config"),
            working: None,
            home: None,
        };
        let result = run_in_registry(
            Output::new(true),
            Change {
                key,
                value: Some(value),
                layer,
            },
            (&places, &registry),
            || panic!("refusal must precede kernel open"),
            |_, _| panic!("refusal must precede journal"),
        );
        assert!(result.is_err(), "{key}");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }
    assert!(validate(&registry, "updates", Some("auto"), LayerName::User).is_ok());
    assert!(validate(&registry, "updates", Some("off"), LayerName::Project).is_ok());
    assert!(validate(&registry, "tone", None, LayerName::Project).is_ok());
    assert!(validate(&registry, "language", Some("auto"), LayerName::Project).is_ok());
}

#[test]
fn catalog_terminal_config_review_uses_full_review_port_and_plain_bytes() {
    use crate::{
        cli::init::flow::{Answer, FlowPort},
        failure::Failure,
    };
    struct ReviewPort<'a> {
        plain: Plain<'a>,
        review: String,
    }
    impl FlowPort for ReviewPort<'_> {
        fn show(&mut self, text: &str) -> Result<(), Failure> {
            self.plain.show(text)
        }
        fn ask(&mut self, label: &str) -> Result<Answer, Failure> {
            self.plain.ask(label)
        }
        fn review_screen(&mut self, text: &str) -> Result<(), Failure> {
            text.clone_into(&mut self.review);
            self.plain.screen(text)
        }
    }
    let mut draft = Draft::new(
        Registry::built_in().unwrap(),
        Layers::default(),
        LayerName::User,
        &[
            "language=zh-Hant-TW".into(),
            "tone=detailed".into(),
            "updates=propose".into(),
        ],
        false,
    )
    .unwrap();
    let mut output = Vec::new();
    let mut input = "\nno\n".as_bytes();
    let mut port = ReviewPort {
        plain: Plain {
            input: &mut input,
            output: &mut output,
        },
        review: String::new(),
    };
    assert!(!collect(&mut port, &mut draft).unwrap());
    let expected = "Review user preferences: language=zh-Hant-TW, tone=detailed, updates=propose";
    assert_eq!(port.review, expected);
    let mut plain_output = Vec::new();
    let mut plain_input = "\nno\n".as_bytes();
    assert!(
        !collect(
            &mut Plain {
                input: &mut plain_input,
                output: &mut plain_output
            },
            &mut draft
        )
        .unwrap()
    );
    assert_eq!(output, plain_output);
    assert_eq!(
        String::from_utf8(output).unwrap().matches(expected).count(),
        1
    );
}
