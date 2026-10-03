use super::super::command::{ApplyChoices, apply_preferences_with_io, prepare_preferences};
use crate::cli::{output::Output, trust, trust_path};
use maestro_catalog::settings::FilePreferences;
use maestro_test_scratch::scratch_directory;
use std::{fs, io::Cursor};

#[test]
fn catalog_presentation_init_terminal_handoff_is_localized_and_blank_declines() {
    let root = scratch_directory().unwrap().canonicalize().unwrap();
    let source = FilePreferences::new(&root, &root);
    let visible = trust_path::visible_path(&root);
    for (language, expected, declined) in [
        (
            "fr",
            format!("Approuver {visible} ? [y/N] "),
            "écriture des préférences seules refusée ; aucun fichier écrit",
        ),
        (
            "es",
            format!("¿Aprobar {visible}? [y/N] "),
            "se rechazó escribir solo las preferencias; no se escribió ningún archivo",
        ),
    ] {
        let output = Output::new(false).with_language(language).unwrap();
        let mut rendered = Vec::new();
        let result = apply_preferences_with_io(
            output,
            &root,
            &ApplyChoices {
                apply: true,
                preferences_only: true,
                non_interactive: false,
                confirm_path: None,
            },
            (&prepare_preferences(&root, &source, &[]).unwrap(), &[]),
            (true, &mut "\n".as_bytes(), &mut rendered),
        );
        assert_eq!(String::from_utf8(rendered).unwrap(), expected);
        assert_eq!(result.unwrap_err().to_string(), declined);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    }
}

#[test]
fn catalog_init_regression_scripted_terminal_approval_never_reads_input() {
    let root = scratch_directory().unwrap().canonicalize().unwrap();
    let source = FilePreferences::new(&root, &root);
    for (json, non_interactive) in [(false, true), (true, false)] {
        let mut input = Cursor::new(b"\n");
        let mut rendered = Vec::new();
        let result = apply_preferences_with_io(
            Output::new(json),
            &root,
            &ApplyChoices {
                apply: true,
                preferences_only: true,
                non_interactive,
                confirm_path: None,
            },
            (&prepare_preferences(&root, &source, &[]).unwrap(), &[]),
            (true, &mut input, &mut rendered),
        );
        assert!(result.is_err());
        assert_eq!(input.position(), 0, "scripted approval consumed input");
        assert!(rendered.is_empty());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    }
}

#[test]
fn catalog_init_regression_exact_confirmation_is_non_prompting_on_a_terminal() {
    let root = scratch_directory().unwrap().canonicalize().unwrap();
    let mut input = Cursor::new(b"must not be read\n");
    let mut rendered = Vec::new();
    let confirmation = trust::approve_preferences_with_io(
        &root,
        Some(&root),
        "must not be shown",
        true,
        (&mut input, &mut rendered),
    )
    .unwrap();
    assert!(confirmation.is_some());
    assert_eq!(input.position(), 0);
    assert!(rendered.is_empty());
}
