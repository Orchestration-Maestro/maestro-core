use super::super::command::{ApplyChoices, PreferenceChoices, preferences_only_with_io};
use crate::cli::{output::Output, trust_path};
use maestro_catalog::settings::FilePreferences;
use maestro_test_scratch::scratch_directory;
use std::fs;

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
        let result = preferences_only_with_io(
            output,
            &root,
            &ApplyChoices {
                apply: true,
                preferences_only: true,
                confirm_path: None,
            },
            PreferenceChoices {
                source: &source,
                choices: &[],
            },
            (true, &mut "\n".as_bytes(), &mut rendered),
        );
        assert_eq!(String::from_utf8(rendered).unwrap(), expected);
        assert_eq!(result.unwrap_err().to_string(), declined);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    }
}
