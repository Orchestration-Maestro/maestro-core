//! GitHub Copilot CLI 1.0.88 on both authoring shapes of ADR-0005: an agent
//! profile carrying a `metadata:` block, and a plain profile beside a
//! `.maestro.toml` sidecar. Copilot runs offline against the scripted model
//! endpoint, with `COPILOT_HOME` and `HOME` in the sandbox, so no account and
//! no real configuration take part.

use super::{
    host_pins::{self, COPILOT, Knowledge},
    host_sandbox::Sandbox,
    provider::{self, MODEL, Provider, Step},
};
use serde_json::json;
use std::{ffi::OsStr, fs, process::Command};

/// The synthetic skill both hosts discover under `.agents/skills`, with an
/// Agent Skills `metadata:` block.
pub(super) const SKILL: &str = "---\nname: probe-skill\n\
    description: Synthetic skill for the catalog host probe, marker probe-skill-description.\n\
    metadata:\n  owner: team-synthetic\n---\nSearch the synthetic glossary first.\n";

/// A control skill with an unknown top-level key (`owner:`), beside the
/// probe skill. Copilot 1.0.88 loads it and reports nothing, so its silence
/// about the probe skill's `metadata:` block is no evidence of support.
const CONTROL_SKILL: &str = "---\nname: control-skill\n\
    description: Synthetic control skill for the catalog host probe, \
    marker control-skill-description.\n\
    owner: team-synthetic\n---\nA control for unknown skill keys.\n";

/// What one Copilot session sent and logged.
struct Session {
    /// The chat requests the scripted endpoint received.
    requests: Vec<serde_json::Value>,
    /// Every line of the session's log files.
    log: String,
    /// The same-name agents the probe found across the project and user
    /// agent directories before the run.
    shadows: Vec<String>,
}

/// Runs one non-interactive Copilot session with the custom agent `probe`
/// in a trusted project holding `files`, and `user_agents` in the sandbox's
/// `COPILOT_HOME/agents`.
fn session(files: &[(&str, &str)], user_agents: &[(&str, &str)]) -> Session {
    let host = host_pins::require(&COPILOT);
    let knowledge = Knowledge::published();
    let provider = Provider::start(vec![Step {
        tool: "maestro-knowledge_search",
        arguments: json!({"collection": "synthetic", "query": "What does the glossary say?"}),
        when: None,
    }]);
    let sandbox = Sandbox::new();
    let servers = json!({"mcpServers": {"maestro": knowledge.server(&json!({"tools": ["*"]}))}});
    sandbox.write("project/.mcp.json", &servers.to_string());
    sandbox.write("project/.agents/skills/probe-skill/SKILL.md", SKILL);
    sandbox.write(
        "project/.agents/skills/control-skill/SKILL.md",
        CONTROL_SKILL,
    );
    for (path, text) in files {
        sandbox.write(&format!("project/.github/agents/{path}"), text);
    }
    for (path, text) in user_agents {
        sandbox.write(&format!("copilot/agents/{path}"), text);
    }
    let copilot_home = sandbox.home().with_file_name("copilot");
    let shadows = host_pins::same_name_agents(&[
        sandbox.project().join(".github/agents"),
        copilot_home.join("agents"),
    ]);
    let mut command = Command::new(&host.executable);
    command.args(["-p", "What does the glossary say?", "--agent", "probe"]);
    command.args(["--no-auto-update", "--allow-all-tools", "-s"]);
    let base_url = provider.base_url();
    let ended = sandbox.run(
        command,
        &[
            ("COPILOT_HOME", copilot_home.as_os_str()),
            ("COPILOT_OFFLINE", OsStr::new("true")),
            ("COPILOT_PROVIDER_BASE_URL", OsStr::new(&base_url)),
            ("COPILOT_MODEL", OsStr::new(MODEL)),
            // Exactly "true" also trusts the project, which loads its MCP servers.
            ("COPILOT_ALLOW_ALL", OsStr::new("true")),
        ],
    );
    assert_eq!(ended.code, Some(0), "{ended:?}");
    let log = fs::read_dir(copilot_home.join("logs"))
        .unwrap()
        .map(|entry| fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect::<String>();
    Session {
        requests: provider.requests(),
        log,
        shadows,
    }
}

impl Session {
    /// Asserts what both shapes share: the agent's body and both skills reach
    /// the model with no diagnostic naming either skill, the tools are exactly
    /// the declared ones plus the two Copilot always adds, and the MCP call
    /// returned Maestro evidence.
    fn assert_agent_ran(&self) {
        let first = &self.requests[0];
        let mut tools = provider::offered_tools(first);
        tools.sort();
        assert_eq!(tools, ["maestro-knowledge_search", "skill", "sql", "view"]);
        let system = provider::system_prompt(first);
        assert!(system.contains("Marker: project-agent."), "{system}");
        assert!(
            system.contains("marker probe-skill-description"),
            "{system}"
        );
        assert!(
            !system.contains("team-synthetic"),
            "metadata reached the prompt"
        );
        let results = provider::tool_results(self.requests.last().unwrap());
        assert!(results.contains("maestro-evidence/1"), "{results}");
        assert!(
            !self.log.contains("probe-skill"),
            "the probe skill drew a diagnostic: {}",
            self.log
        );
        assert!(
            system.contains("marker control-skill-description"),
            "{system}"
        );
        assert!(
            !self.log.contains("control-skill"),
            "the control skill's unknown key drew a diagnostic: {}",
            self.log
        );
        assert!(self.shadows.is_empty(), "{:?}", self.shadows);
    }
}

#[test]
#[ignore = "runs the installed GitHub Copilot CLI 1.0.88 offline in a sandbox; run explicitly"]
fn catalog_host_probe_copilot_ignores_a_metadata_block_with_a_warning() {
    let session = session(
        &[("probe.agent.md", &host_pins::fixture("metadata.agent.md"))],
        &[],
    );
    session.assert_agent_ran();
    assert!(
        session
            .log
            .contains(".github/agents/probe.agent.md: unknown field ignored: metadata"),
        "{}",
        session.log
    );
    println!("copilot metadata shape: loaded; `metadata` reported as an unknown field");
}

#[test]
#[ignore = "runs the installed GitHub Copilot CLI 1.0.88 offline in a sandbox; run explicitly"]
fn catalog_host_probe_copilot_loads_a_sidecar_profile_without_a_diagnostic() {
    let session = session(
        &[
            ("probe.agent.md", &host_pins::fixture("sidecar.agent.md")),
            (
                "probe.maestro.toml",
                &host_pins::fixture("sidecar.maestro.toml"),
            ),
        ],
        &[],
    );
    session.assert_agent_ran();
    assert!(!session.log.contains("probe.agent.md"), "{}", session.log);
    assert!(
        !session.log.contains("probe.maestro.toml"),
        "{}",
        session.log
    );
    assert!(!session.log.contains("unknown field"), "{}", session.log);
    println!("copilot sidecar shape: loaded without a diagnostic");
}

#[test]
#[ignore = "runs the installed GitHub Copilot CLI 1.0.88 offline in a sandbox; run explicitly"]
fn catalog_host_probe_copilot_lets_a_same_name_user_agent_shadow_the_project() {
    let project = host_pins::fixture("sidecar.agent.md");
    let user = project.replace("Marker: project-agent.", "Marker: user-agent.");
    let session = session(
        &[("probe.agent.md", &project)],
        &[("renamed.agent.md", &user)],
    );
    assert_eq!(session.shadows, ["probe"]);
    let system = provider::system_prompt(&session.requests[0]);
    assert!(system.contains("Marker: user-agent."), "{system}");
    assert!(!system.contains("Marker: project-agent."), "{system}");
    assert!(!session.log.contains("renamed.agent.md"), "{}", session.log);
    assert!(!session.log.contains("probe.agent.md"), "{}", session.log);
    println!("copilot shadow: the user agent wins silently; the probe detects it");
}
