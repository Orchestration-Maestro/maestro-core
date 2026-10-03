//! `maestro config`: the user file `preferences.toml`, the project file
//! `.maestro/config.toml` found from the working directory, `--set`, their
//! explanation and their journaled history; the kernel's `config.toml` is
//! never read or written; `doctor` names a refused key; and models off.

use super::support::{Home, initialize_mcp, make_safe_preferences_path};
use serde_json::{Value, json};
use std::{fs, io::Write as _, path::PathBuf};

/// The schema line every preferences file starts with.
const SCHEMA: &str = "schema = \"maestro-preferences/1\"\n";

/// A project directory in `home`, `work/project`, with its project file
/// holding `body` after its schema.
fn project(home: &Home, body: &str) -> PathBuf {
    let directory = home.root().join("work").join("project");
    fs::create_dir_all(directory.join(".maestro")).unwrap();
    fs::write(
        directory.join(".maestro").join("config.toml"),
        format!("{SCHEMA}{body}"),
    )
    .unwrap();
    make_safe_preferences_path(&directory.join(".maestro"));
    make_safe_preferences_path(&directory.join(".maestro/config.toml"));
    directory
}

#[test]
fn set_get_unset_and_history_keep_the_grants_file_untouched() {
    let home = Home::new();
    let grants = fs::read(home.config().join("config.toml")).unwrap();
    let set = home.run(&["config", "set", "tone", "brief"]);
    assert_eq!(set.code, Some(0), "{set:?}");
    let user = home.config().join("preferences.toml");
    assert_eq!(
        set.stdout.trim(),
        format!("tone = \"brief\" in {} (was unset)", user.display())
    );
    assert_eq!(
        fs::read_to_string(&user).unwrap(),
        format!("{SCHEMA}tone = \"brief\"\n")
    );
    let got = home.run(&["config", "get", "tone"]);
    assert_eq!((got.code, got.stdout.trim()), (Some(0), "brief"));
    let got = home.run(&["--json", "config", "get", "tone"]);
    assert_eq!(
        got.json(),
        json!({
            "schema": "maestro-cli/config-get/1",
            "key": "tone",
            "value": "brief",
            "source": {"layer": "user", "path": user},
            "class": "free",
            "diagnostics": [],
        })
    );
    let unset = home.run(&["config", "unset", "tone"]);
    assert_eq!(unset.code, Some(0), "{unset:?}");
    assert_eq!(fs::read_to_string(&user).unwrap(), SCHEMA);
    let history = home.run(&["--json", "config", "history"]);
    assert_eq!(history.code, Some(0), "{history:?}");
    let history_text = home.run(&["config", "history"]);
    assert_eq!(history_text.code, Some(0), "{history_text:?}");
    assert!(history_text.stdout.contains("tone: unset -> \"brief\""));
    assert!(history_text.stdout.contains("tone: \"brief\" -> unset"));
    let changes: Vec<Value> = history.json()["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| {
            json!([
                change["principal"],
                change["key"],
                change["old"],
                change["new"],
                change["file"]
            ])
        })
        .collect();
    assert_eq!(
        changes,
        vec![
            json!(["local", "tone", null, "brief", user]),
            json!(["local", "tone", "brief", null, user]),
        ]
    );
    assert_eq!(fs::read(home.config().join("config.toml")).unwrap(), grants);
}

/// The layers of `home`: a user file setting `tone` and `language`, and a
/// project whose file sets `language`; returns a directory nested in the
/// project and the project file.
fn layered(home: &Home) -> (PathBuf, PathBuf) {
    fs::write(
        home.config().join("preferences.toml"),
        format!("{SCHEMA}tone = \"brief\"\nlanguage = \"es\"\n"),
    )
    .unwrap();
    let directory = project(home, "# ours\nlanguage = \"fr\"\n");
    let nested = directory.join("src");
    fs::create_dir_all(&nested).unwrap();
    let file = directory.join(".maestro").join("config.toml");
    (nested, file.canonicalize().unwrap())
}

#[test]
fn explain_names_the_layer_of_each_value_a_flag_then_the_project_then_the_user_file() {
    let home = Home::new();
    let (nested, project_file) = layered(&home);
    let explained = home.run_in(
        &nested,
        &["--json", "--set", "search.k=20", "config", "explain"],
    );
    assert_eq!(explained.code, Some(0), "{explained:?}");
    let document = explained.json();
    assert_eq!(document["files"]["project"]["path"], json!(project_file));
    let setting = |key: &str| {
        document["settings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|setting| setting["key"] == key)
            .unwrap()
            .clone()
    };
    assert_eq!(setting("language")["value"], "fr");
    assert_eq!(setting("language")["source"]["layer"], "project");
    assert_eq!(setting("language")["overridden"][0]["value"], "es");
    assert_eq!(setting("tone")["source"]["layer"], "user");
    assert_eq!(setting("search.k")["value"], 20);
    assert_eq!(setting("search.k")["source"], json!({"layer": "--set"}));
    assert_eq!(
        setting("search.rerank.depth")["source"],
        json!({"layer": "default"})
    );
    assert_eq!(setting("tone")["class"], "free");
}

#[test]
fn set_in_the_project_edits_the_file_in_place_and_list_shows_its_layer() {
    let home = Home::new();
    let (nested, project_file) = layered(&home);
    let text = home.run_in(&nested, &["config", "explain", "language"]);
    assert_eq!(text.code, Some(0), "{text:?}");
    assert!(text.stdout.contains(&format!(
        "user file: {}",
        home.config().join("preferences.toml").display()
    )));
    assert!(
        text.stdout
            .contains(&format!("project file: {}", project_file.display()))
    );
    assert!(
        text.stdout
            .contains("language = \"fr\"\n  set by: workspace\n  class: free"),
        "{}",
        text.stdout
    );
    let set = home.run_in(&nested, &["config", "set", "--project", "tone", "detailed"]);
    assert_eq!(set.code, Some(0), "{set:?}");
    assert_eq!(
        fs::read_to_string(&project_file).unwrap(),
        format!("{SCHEMA}# ours\nlanguage = \"fr\"\ntone = \"detailed\"\n")
    );
    let listed = home.run_in(&nested, &["config", "list"]);
    assert!(
        listed
            .stdout
            .contains("tone = \"detailed\"  (workspace; class free)\n"),
        "{}",
        listed.stdout
    );
}

#[test]
fn restrictive_resolution_is_used_by_config_and_locked_settings_refuse_mutation() {
    let home = Home::new();
    fs::write(
        home.config().join("preferences.toml"),
        format!("{SCHEMA}ask.output_tokens = 100\nupdates = \"propose\"\n"),
    )
    .unwrap();
    let explained = home.run(&[
        "--json",
        "--set",
        "ask.output_tokens=off",
        "config",
        "explain",
        "ask.output_tokens",
    ]);
    assert_eq!(explained.code, Some(0), "{explained:?}");
    let setting = &explained.json()["settings"][0];
    assert_eq!(setting["value"], 100);
    assert_eq!(setting["class"], "bounded");
    assert_eq!(setting["source"]["layer"], "user");
    assert!(
        setting["diagnostics"][0]
            .as_str()
            .unwrap()
            .contains("flag budget widening")
    );

    let listed = home.run(&["--json", "config", "list"]);
    assert_eq!(listed.code, Some(0), "{listed:?}");
    let listed_json = listed.json();
    let raw = listed_json["settings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|setting| setting["key"] == "raw_prompt_logging")
        .unwrap();
    assert_eq!(raw["class"], "locked");

    let set = home.run(&["config", "set", "raw_prompt_logging", "true"]);
    assert_eq!(set.code, Some(2), "{set:?}");
    let flag = home.run(&["--set", "evidence_validation=false", "config", "list"]);
    assert_eq!(flag.code, Some(2), "{flag:?}");
}

#[test]
fn a_refused_key_is_named_by_config_and_by_doctor() {
    let home = Home::new();
    fs::write(
        home.config().join("preferences.toml"),
        format!("{SCHEMA}[search]\nfoo = 1\n"),
    )
    .unwrap();
    let user = home.config().join("preferences.toml");
    let listed = home.run(&["config", "list"]);
    assert_eq!(listed.code, Some(2), "{listed:?}");
    assert_eq!(
        listed.stderr.trim(),
        format!("{}: unknown key \"search.foo\"", user.display())
    );
    let doctor = home.run(&["--json", "doctor"]);
    assert_eq!(doctor.code, Some(1), "{doctor:?}");
    let document = doctor.json();
    let settings = document["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "settings")
        .unwrap();
    assert_eq!(
        settings["detail"],
        format!("{}: unknown key \"search.foo\"", user.display())
    );
    assert_eq!(settings["passed"], false);
    assert!(settings["next_action"].as_str().unwrap().contains("fix"));
    fs::remove_file(&user).unwrap();
    let refused = home.run(&["config", "set", "search.k", "0"]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(
        refused.stderr.trim(),
        "search.k: expected a whole number from 1 to 50"
    );
}

#[test]
fn models_off_refuses_ask_on_the_command_line_and_over_mcp() {
    let home = Home::bare();
    let asked = home.run(&[
        "--json",
        "--set",
        "models.compute=off",
        "knowledge",
        "ask",
        "--collection",
        "docs",
        "--question",
        "How is the service configured?",
    ]);
    assert_eq!(asked.code, Some(2), "{asked:?}");
    assert_eq!(
        asked.json(),
        json!({
            "schema": "maestro-cli/knowledge-ask-error/1",
            "error": {
                "code": "models_off",
                "message": "models.compute is off: ask needs a model; use knowledge search",
            }
        })
    );
    for command in ["prepare", "publish"] {
        let arguments = [
            "--set",
            "models.compute=off",
            "knowledge",
            command,
            "--collection",
            "docs",
            "--card",
            "0000",
        ];
        let message = format!("models.compute is off: {command} calls the model router");
        let refused = home.run(&arguments);
        assert_eq!(refused.code, Some(2), "{refused:?}");
        assert_eq!(refused.stderr.trim(), message);
        let refused = home.run(&[&["--json"][..], &arguments].concat());
        assert_eq!(refused.code, Some(2), "{refused:?}");
        assert_eq!(
            refused.json(),
            json!({
                "schema": format!("maestro-cli/knowledge-{command}-error/1"),
                "error": {"code": "models_off", "message": message},
            })
        );
    }
    assert_eq!(
        fs::read_dir(home.data()).unwrap().count(),
        0,
        "no kernel is opened before the refusal"
    );

    let (child, mut input) = home.start_with_stdin(&["--set", "models.compute=off", "mcp"]);
    input
        .write_all(
            concat!(
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{",
                "\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},",
                "\"clientInfo\":{\"name\":\"test\",\"version\":\"1\"}}}\n",
                "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{",
                "\"name\":\"knowledge_ask\",\"arguments\":{\"collection\":\"docs\",",
                "\"question\":\"How can I configure the service?\"}}}\n",
            )
            .as_bytes(),
        )
        .unwrap();
    drop(input);
    let ended = child.finish();
    assert_eq!(ended.code, Some(0), "{ended:?}");
    let reply: Value = ended
        .stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|message| message["id"] == 2)
        .unwrap();
    assert_eq!(reply["result"]["isError"], true, "{reply}");
    let error: Value =
        serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(error["error"], asked.json()["error"]);
}

#[test]
fn mcp_warns_and_uses_user_defaults_for_a_workspace_outside_home() {
    let home = Home::bare();
    let outside = home.root().parent().unwrap().to_path_buf();
    let (child, mut input) = home.start_with_stdin(&[
        "--set",
        "models.compute=off",
        "mcp",
        "--workspace",
        outside.to_str().unwrap(),
    ]);
    initialize_mcp(&mut input);
    drop(input);
    let served = child.finish();
    assert_eq!(served.code, Some(0), "{served:?}");
    assert!(served.stderr.contains("maestro trust add"), "{served:?}");
}

#[test]
fn a_malformed_set_flag_stops_a_command_that_writes() {
    let home = Home::bare();
    for arguments in [
        &["--set", "search.typo=1", "config", "set", "tone", "brief"][..],
        &["--set", "search.typo=1", "config", "unset", "tone"],
        &["--set", "search.k=0", "config", "history"],
    ] {
        let refused = home.run(arguments);
        assert_eq!(refused.code, Some(2), "{refused:?}");
        assert!(refused.stderr.starts_with("--set "), "{refused:?}");
    }
    assert!(!home.config().join("preferences.toml").exists());
    assert_eq!(fs::read_dir(home.data()).unwrap().count(), 0);
}

#[test]
fn mcp_resolves_a_relative_workspace_and_refuses_a_file() {
    let home = Home::bare();
    let directory = project(&home, "search.foo = 1\n");
    fs::write(directory.join("README.md"), "# project\n").unwrap();
    let work = home.root().join("work");
    let read = home.run_in(&work, &["mcp", "--workspace", "project"]);
    assert_eq!(read.code, Some(2), "{read:?}");
    assert!(
        read.stderr.trim().ends_with("unknown key \"search.foo\""),
        "the relative workspace's project file is read: {read:?}"
    );
    let file = home.run_in(&work, &["mcp", "--workspace", "project/README.md"]);
    assert_eq!(file.code, Some(2), "{file:?}");
    assert_eq!(
        file.stderr.trim(),
        "--workspace project/README.md: the path is not a directory: no project file is read"
    );
}

#[test]
fn config_raw_json_keeps_pre_cedar_bytes() {
    let home = Home::new();
    let output = home.run(&["config", "get", "tone", "--json"]);
    assert_eq!(output.code, Some(0), "{output:?}");
    assert_eq!(
        output.stdout,
        concat!(
            "{\"class\":\"free\",\"diagnostics\":[],\"key\":\"tone\",",
            "\"schema\":\"maestro-cli/config-get/1\",\"source\":{\"layer\":\"default\"},",
            "\"value\":\"normal\"}\n",
        )
    );
}
