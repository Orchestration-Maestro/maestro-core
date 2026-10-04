//! Claude Code 2.1.283 and codex-cli 0.150.1 receive MCP registration
//! guidance only (FR-S3-007), not agent projection: the probe checks that the
//! documented registration command records `maestro mcp` in a sandboxed
//! configuration directory, and whether the host connects to it.

use super::{
    host_pins::{self, CLAUDE_CODE, CODEX, Knowledge},
    host_sandbox::Sandbox,
};
use serde_json::{Value, json};
use std::{ffi::OsStr, fs, process::Command};

#[test]
#[ignore = "runs the installed Claude Code 2.1.283 in a sandbox; run explicitly"]
fn catalog_host_probe_claude_code_connects_a_local_registration_only() {
    let host = host_pins::require(&CLAUDE_CODE);
    let knowledge = Knowledge::published();
    let sandbox = Sandbox::new();
    let config = sandbox.home().with_file_name("claude");
    let settings = [
        ("CLAUDE_CONFIG_DIR", config.as_os_str()),
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", OsStr::new("1")),
        ("DISABLE_AUTOUPDATER", OsStr::new("1")),
    ];
    let claude = |arguments: &[&str]| {
        let mut command = Command::new(&host.executable);
        command.args(arguments);
        sandbox.run(command, &settings)
    };
    let servers = json!({"mcpServers": {"maestro": knowledge.server(&json!({}))}});
    sandbox.write("project/.mcp.json", &servers.to_string());
    let pending = claude(&["mcp", "list"]);
    assert_eq!(pending.code, Some(0), "{pending:?}");
    assert!(pending.stdout.contains("Pending approval"), "{pending:?}");

    let server = knowledge.server(&json!({})).to_string();
    let added = claude(&[
        "mcp",
        "add-json",
        "--scope",
        "local",
        "maestro-local",
        &server,
    ]);
    assert_eq!(added.code, Some(0), "{added:?}");
    let listed = claude(&["mcp", "list"]);
    let local = listed
        .stdout
        .lines()
        .find(|line| line.starts_with("maestro-local:"))
        .unwrap_or_else(|| panic!("{listed:?}"));
    assert!(local.ends_with("✔ Connected"), "{local}");
    println!("claude code: local registration connects; project .mcp.json awaits approval");
}

#[test]
#[ignore = "runs the installed codex-cli 0.150.1 in a sandbox; run explicitly"]
fn catalog_host_probe_codex_records_a_registration_without_connecting() {
    let host = host_pins::require(&CODEX);
    let knowledge = Knowledge::published();
    let sandbox = Sandbox::new();
    let codex_home = sandbox.home().with_file_name("codex");
    fs::create_dir_all(&codex_home).unwrap();
    let settings = [("CODEX_HOME", codex_home.as_os_str())];
    let codex = |arguments: &[&str]| {
        let mut command = Command::new(&host.executable);
        command.args(arguments);
        sandbox.run(command, &settings)
    };
    let server = knowledge.server(&json!({}));
    let variables = server["env"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, value)| format!("{name}={}", value.as_str().unwrap()))
        .collect::<Vec<_>>();
    let mut arguments = vec!["mcp", "add", "maestro"];
    for variable in &variables {
        arguments.extend(["--env", variable]);
    }
    arguments.extend(["--", env!("CARGO_BIN_EXE_maestro"), "mcp"]);
    let added = codex(&arguments);
    assert_eq!(added.code, Some(0), "{added:?}");
    let shown = codex(&["mcp", "get", "maestro", "--json"]);
    assert_eq!(shown.code, Some(0), "{shown:?}");
    let shown = serde_json::from_str::<Value>(&shown.stdout).unwrap();
    assert_eq!(shown["enabled"], true);
    assert_eq!(shown["transport"]["type"], "stdio");
    assert_eq!(shown["transport"]["command"], env!("CARGO_BIN_EXE_maestro"));
    assert_eq!(shown["transport"]["args"], json!(["mcp"]));
    assert_eq!(shown["transport"]["env"], server["env"]);
    println!("codex: registration recorded; `codex mcp` never connects (live call not run)");
}
