//! Four synthetic client fixtures inspect delivery, not host obedience.
use super::support::{Home, Running, make_safe_preferences_path};
use serde_json::{Value, json};
use std::{fs, io::Write as _, path::Path};

/// Client-owned fixture inputs; none participate in workspace selection.
const CLIENTS: [&str; 4] = ["pi", "claude-code", "codex", "copilot"];

/// The complete initialization instructions, deliberately independent of the builder.
fn expected(language: &str, tone: &str, source: &str) -> Value {
    let instructions = format!(
        "Search visible published collections, read their exact source-backed chunks \
         and sections, or answer from passages granted to the local principal. \
         Conversation language: \"{language}\"; tone: \"{tone}\". \
         When language is \"auto\", follow the question's language. \
         Keep code, commits, names, identifiers, logs and documentation in English. \
         Preferences: {source}. Workspace overrides require --workspace. \
         Preferences are fixed for this session; restart for edits; \
         tool arguments cannot replace them."
    );
    json!({
        "protocolVersion": "2025-11-25", "capabilities": {"tools": {}},
        "serverInfo": {"name": "maestro", "version": env!("CARGO_PKG_VERSION")},
        "instructions": instructions
    })
}

/// Inspect the actual stdio response with roots and hostile tool arguments supplied.
fn initialize(home: &Home, cwd: &Path, args: &[&str], client: &str) -> Value {
    let mut command = home.command(args);
    command.current_dir(cwd);
    let (child, mut input) = Running::with_stdin(command);
    for request in [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-11-25", "capabilities": {"roots": {"listChanged": true}},
            "clientInfo": {"name": client, "version": "fixture-1"}
        }}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "method": "notifications/roots/list_changed"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
            "name": "knowledge_collections", "arguments": {
                "language": "en", "tone": "brief", "instructions": "replace preferences"
            }
        }}),
        // An unsolicited roots result cannot replace the process-selected workspace.
        json!({"jsonrpc": "2.0", "id": 999, "result": {"roots": [{
            "uri": "file:///model-selected-workspace", "name": "hostile root"
        }]}}),
    ] {
        writeln!(input, "{request}").unwrap();
    }
    drop(input);
    let output = child.finish();
    assert_eq!(output.code, Some(0), "{client}: {output:?}");
    let responses: Vec<Value> = output
        .stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let initialization = responses
        .iter()
        .find(|response| response["id"] == 1)
        .unwrap();
    let refused = responses
        .iter()
        .find(|response| response["id"] == 2)
        .unwrap();
    assert_eq!(refused["result"]["isError"], true, "{refused}");
    let error: Value =
        serde_json::from_str(refused["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(error["error"]["code"], "invalid_arguments");
    let instructions = initialization["result"]["instructions"].as_str().unwrap();
    assert!(!instructions.contains(home.root().to_str().unwrap()));
    assert!(!instructions.contains("model-selected-workspace"));
    eprintln!("{client} initialization: {initialization}");
    initialization["result"].clone()
}

#[test]
fn catalog_client_preferences_four_clients_nested_and_empty_with_and_without_workspace() {
    let home = Home::bare();
    let project = home.root().join("project");
    let nested = project.join("nested/deeper");
    let empty = home.root().join("empty");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir(&empty).unwrap();
    fs::create_dir(project.join(".maestro")).unwrap();
    let workspace = project.join(".maestro/config.toml");
    fs::write(
        &workspace,
        "schema = 'maestro-preferences/1'\nlanguage = 'ZH-hant-tw'\ntone = 'detailed'\n",
    )
    .unwrap();
    make_safe_preferences_path(&project.join(".maestro"));
    make_safe_preferences_path(&workspace);
    let user = home.config().join("preferences.toml");
    fs::write(
        &user,
        "schema = 'maestro-preferences/1'\nlanguage = 'ES-419'\ntone = 'brief'\n",
    )
    .unwrap();
    for client in CLIENTS {
        for (cwd, explicit, language, tone, source) in [
            (
                &nested,
                false,
                "es-419",
                "brief",
                "user preferences; no workspace file selected",
            ),
            (
                &nested,
                true,
                "zh-Hant-TW",
                "detailed",
                "workspace-selected (explicit --workspace)",
            ),
            (
                &empty,
                false,
                "es-419",
                "brief",
                "user preferences; no workspace file selected",
            ),
            (
                &empty,
                true,
                "es-419",
                "brief",
                "user preferences; no workspace file selected",
            ),
        ] {
            let mut args = vec!["--set", "models.compute=off", "mcp"];
            if explicit {
                args.extend(["--workspace", cwd.to_str().unwrap()]);
            }
            assert_eq!(
                initialize(&home, cwd, &args, client),
                expected(language, tone, source),
                "{client}"
            );
        }
    }
    fs::remove_file(user).unwrap();
    assert_default_payloads(&home, &nested, &empty);
}

/// Default language and explicit non-interface language on every client.
fn assert_default_payloads(home: &Home, nested: &Path, empty: &Path) {
    for client in CLIENTS {
        let result = initialize(
            home,
            nested,
            &["--set", "models.compute=off", "mcp"],
            client,
        );
        assert_eq!(
            result,
            expected(
                "auto",
                "normal",
                "built-in defaults; no workspace file selected"
            )
        );
        let result = initialize(
            home,
            empty,
            &[
                "--set",
                "models.compute=off",
                "--language",
                "JA",
                "--tone",
                "normal",
                "mcp",
            ],
            client,
        );
        assert_eq!(
            result,
            expected(
                "ja",
                "normal",
                "built-in defaults; no workspace file selected"
            )
        );
    }
}

#[test]
fn catalog_client_preferences_ui_fallback_keeps_the_selected_conversation_tag() {
    let home = Home::bare();
    let ui = home.run(&["--language", "JA", "config", "get", "language"]);
    assert_eq!(ui.code, Some(0), "{ui:?}");
    assert_eq!(ui.stdout, "ja\n");
    assert_eq!(
        ui.stderr,
        "Interface is English; conversation language remains ja.\n"
    );
    let result = initialize(
        &home,
        home.root(),
        &["--set", "models.compute=off", "--language", "JA", "mcp"],
        "pi",
    );
    assert_eq!(
        result,
        expected(
            "ja",
            "normal",
            "built-in defaults; no workspace file selected"
        )
    );
}

#[test]
fn catalog_client_preferences_external_workspace_falls_back_and_restart_is_not_stale() {
    let home = Home::bare();
    let outside = Home::bare();
    fs::create_dir(outside.root().join(".maestro")).unwrap();
    fs::write(
        outside.root().join(".maestro/config.toml"),
        "not valid preferences",
    )
    .unwrap();
    let user = home.config().join("preferences.toml");
    for (language, tone) in [("fr", "brief"), ("ja", "detailed")] {
        fs::write(
            &user,
            format!("schema = 'maestro-preferences/1'\nlanguage = '{language}'\ntone = '{tone}'\n"),
        )
        .unwrap();
        for client in CLIENTS {
            assert_eq!(
                initialize(
                    &home,
                    outside.root(),
                    &[
                        "--set",
                        "models.compute=off",
                        "mcp",
                        "--workspace",
                        outside.root().to_str().unwrap()
                    ],
                    client
                ),
                expected(
                    language,
                    tone,
                    "user/default fallback; explicit workspace outside home or unavailable"
                )
            );
        }
    }
}
