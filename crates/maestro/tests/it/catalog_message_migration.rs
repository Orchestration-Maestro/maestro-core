//! Slice-one process neighbours: localization never changes repair or machine semantics.
use super::{
    catalog_presentation::{TONES, project},
    support::Home,
};
use std::fs;

#[test]
fn catalog_presentation_trust_ignores_malformed_config_and_invalid_language() {
    let home = Home::bare();
    let root = project(&home);
    fs::write(
        home.config().join("preferences.toml"),
        "invalid preferences",
    )
    .unwrap();
    home.configure("invalid authority config");
    for (flags, expected) in [
        (
            vec!["--language", "fr"],
            "Réponse de l’espace de travail enregistrée pour",
        ),
        (
            vec!["--language", "INVALID_!"],
            "Recorded workspace answer for",
        ),
        (
            vec!["--set", "language=es", "--language", "fr"],
            "Réponse de l’espace de travail enregistrée pour",
        ),
        (
            vec!["--set", "language=fr", "--set", "language=es"],
            "Respuesta del espacio de trabajo registrada para",
        ),
    ] {
        let mut args = flags;
        args.extend(["trust", "remove", root.to_str().unwrap()]);
        let result = home.run(&args);
        assert_eq!(result.code, Some(0), "{result:?}");
        assert!(result.stdout.starts_with(expected), "{result:?}");
        assert_eq!(result.stderr, "");
    }
}

#[test]
fn catalog_presentation_catalog_failures_localize_frames_not_downstream_data() {
    let home = Home::bare();
    let root = project(&home);
    fs::create_dir_all(root.join("marketplace/index.json")).unwrap();
    for (language, expected) in [
        (
            "en",
            "marketplace/index.json: generated output must be a regular file\n",
        ),
        (
            "fr",
            "marketplace/index.json : la sortie générée doit être un fichier ordinaire\n",
        ),
        (
            "es",
            "marketplace/index.json: la salida generada debe ser un archivo regular\n",
        ),
    ] {
        for tone in TONES {
            let result = home.run(&[
                "--language",
                language,
                "--tone",
                tone,
                "catalog",
                "index",
                "--catalog-dir",
                root.to_str().unwrap(),
                "--check",
            ]);
            assert_eq!(result.code, Some(2), "{result:?}");
            assert_eq!(result.stderr, expected);
            assert_eq!(result.stdout, "");
            let json = home.run(&[
                "--json",
                "--language",
                language,
                "catalog",
                "index",
                "--catalog-dir",
                root.to_str().unwrap(),
                "--check",
            ]);
            assert_eq!(json.code, Some(2), "{json:?}");
            assert_eq!(
                json.stderr,
                "marketplace/index.json: generated output must be a regular file\n"
            );
        }
    }
}

#[test]
fn catalog_presentation_pre_session_failures_preserve_explicit_language_and_exit() {
    let home = Home::bare();
    let missing = home.root().join("missing");
    let path = missing.to_str().unwrap();
    for (language, detail) in [
        ("en", "the path is not a directory: no project file is read"),
        (
            "fr",
            "le chemin n’est pas un dossier : aucun fichier de projet n’est lu",
        ),
        (
            "es",
            "la ruta no es un directorio: no se lee ningún archivo de proyecto",
        ),
    ] {
        let result = home.run(&["--language", language, "mcp", "--workspace", path]);
        assert_eq!(result.code, Some(2), "{result:?}");
        let separator = if language == "fr" { " : " } else { ": " };
        assert_eq!(
            result.stderr,
            format!("--workspace {path}{separator}{detail}\n")
        );
        assert_eq!(result.stdout, "");
    }
}

#[test]
fn catalog_presentation_models_off_keeps_json_english_and_localizes_human_refusals() {
    let home = Home::bare();
    for (language, wording) in [
        (
            "en",
            "models.compute is off: {command} calls the model router\n",
        ),
        (
            "fr",
            "models.compute est off : {command} appelle le routeur de modèles\n",
        ),
        (
            "es",
            "models.compute está off: {command} llama al enrutador de modelos\n",
        ),
    ] {
        for command in ["prepare", "publish"] {
            let args = [
                "--language",
                language,
                "--set",
                "models.compute=off",
                "knowledge",
                command,
                "--collection",
                "docs",
                "--card",
                "0000",
            ];
            let human = home.run(&args);
            assert_eq!(human.code, Some(2), "{human:?}");
            assert_eq!(human.stderr, wording.replace("{command}", command));
            let machine = home.run(&[&["--json"][..], &args].concat());
            assert_eq!(machine.code, Some(2), "{machine:?}");
            assert_eq!(machine.stderr, "");
            assert_eq!(
                machine.stdout,
                format!(
                    concat!(
                        "{{\"schema\":\"maestro-cli/knowledge-{command}-error/1\",",
                        "\"error\":{{\"code\":\"models_off\",\"message\":",
                        "\"models.compute is off: {command} calls the model router\"}}}}\n"
                    ),
                    command = command
                )
            );
        }
    }
    assert_eq!(fs::read_dir(home.data()).unwrap().count(), 0);
}

#[test]
fn catalog_presentation_trust_fallback_note_is_once_and_never_machine_output() {
    let home = Home::bare();
    fs::write(
        home.config().join("preferences.toml"),
        "invalid preferences",
    )
    .unwrap();
    for language in ["en", "fr", "es", "ja", "INVALID_!"] {
        let result = home.run(&["--language", language, "trust", "list"]);
        assert_eq!(result.code, Some(0), "{result:?}");
        assert_eq!(result.stdout, "[]\n");
        assert_eq!(
            result.stderr,
            if language == "ja" {
                "Interface is English; conversation language remains ja.\n"
            } else {
                ""
            }
        );
        let json = home.run(&["--json", "--language", language, "trust", "list"]);
        assert_eq!(json.code, Some(0), "{json:?}");
        assert_eq!(json.stdout, "[]\n");
        assert_eq!(json.stderr, "");
    }
}
