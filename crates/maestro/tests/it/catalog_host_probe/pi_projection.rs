//! Pi 0.87.1 with its installed agent and MCP providers, pi-subagents 0.64.0
//! and pi-mcp-adapter 2.37.0, loaded read-only by path: the Pi projection of
//! the probe agent names its tools, MCP provider, model and skill explicitly.
//! Pi runs against the scripted model endpoint with `PI_CODING_AGENT_DIR` and
//! `HOME` in the sandbox, so no account and no real configuration take part.

use super::{
    copilot_formats::SKILL,
    host_pins::{self, Blocked, Knowledge, PI},
    host_sandbox::Sandbox,
    provider::{self, MODEL, Provider, Step},
};
use serde_json::{Value, json};
use std::{env, ffi::OsStr, path::PathBuf, process::Command};

/// A sentence of the probe agent's body: only the child's prompt holds it.
const AGENT_BODY: &str = "Answer questions about the public synthetic glossary.";

/// The model the `pi.md` projection names for the child; the parent session
/// runs on [`MODEL`].
const CHILD_MODEL: &str = "probe-child-model";

/// The directory holding Pi's installed packages: `MAESTRO_PROBE_PI_PACKAGES`,
/// or else where `pi install` puts them under the user's home.
fn pi_packages() -> PathBuf {
    env::var_os("MAESTRO_PROBE_PI_PACKAGES").map_or_else(
        || {
            PathBuf::from(env::var_os("HOME").unwrap_or_default())
                .join(".pi/agent/npm/node_modules")
        },
        PathBuf::from,
    )
}

/// What one Pi session sent.
struct Session {
    /// The chat requests the scripted endpoint received, parent and child.
    requests: Vec<Value>,
    /// The same-name agents the probe found across the project and user
    /// agent directories before the run.
    shadows: Vec<String>,
    /// Everything Pi printed on standard error.
    stderr: String,
}

impl Session {
    /// The requests of the child agent, whose system prompt holds its body.
    fn child(&self) -> Vec<&Value> {
        self.requests
            .iter()
            .filter(|request| provider::system_prompt(request).contains(AGENT_BODY))
            .collect()
    }

    /// The model every request names, in arrival order, split into the
    /// parent's and the child's.
    fn models(&self) -> (Vec<String>, Vec<String>) {
        let (child, parent): (Vec<&Value>, Vec<&Value>) = self
            .requests
            .iter()
            .partition(|request| provider::system_prompt(request).contains(AGENT_BODY));
        let names = |requests: Vec<&Value>| {
            requests
                .into_iter()
                .map(|request| request["model"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        (names(parent), names(child))
    }

    /// Asserts that Pi printed no diagnostic naming an agent file.
    fn assert_no_agent_diagnostic(&self) {
        for file in ["probe.md", "renamed.md"] {
            assert!(!self.stderr.contains(file), "{}", self.stderr);
        }
    }
}

/// Runs one non-interactive Pi session whose model starts the `probe`
/// subagent of `agent` (the `pi.md` fixture, adapted), with `user_agent` in
/// the sandbox's user agent directory when given.
fn session(agent: &dyn Fn(&str) -> String, user_agent: Option<&str>) -> Session {
    let host = host_pins::require(&PI);
    let packages = pi_packages();
    let [adapter, subagents] =
        [("pi-mcp-adapter", "2.37.0"), ("pi-subagents", "0.64.0")].map(|(name, version)| {
            host_pins::pi_package(&packages, name, version)
                .unwrap_or_else(|Blocked(reason)| panic!("blocked: {reason}"))
                .join("index.ts")
        });
    let knowledge = Knowledge::published();
    let provider = Provider::start(vec![
        Step {
            tool: "subagent",
            arguments: json!({"agent": "probe", "task": "What does the glossary say?",
                "async": false}),
            when: None,
        },
        Step {
            tool: "maestro_knowledge_search",
            arguments: json!({"collection": "synthetic", "query": "What does the glossary say?"}),
            when: Some(AGENT_BODY),
        },
    ]);
    let sandbox = Sandbox::new();
    let agent_dir = sandbox.home().with_file_name("pi-agent");
    let models = json!({"providers": {"probe": {"baseUrl": provider.base_url(),
        "api": "openai-completions", "apiKey": "synthetic-not-a-secret",
        "models": [{"id": MODEL}, {"id": CHILD_MODEL}]}}});
    sandbox.write("pi-agent/models.json", &models.to_string());
    let servers =
        json!({"mcpServers": {"maestro": knowledge.server(&json!({"directTools": true}))}});
    sandbox.write("project/.mcp.json", &servers.to_string());
    sandbox.write("project/.agents/skills/probe-skill/SKILL.md", SKILL);
    let adapted = |text: &str| agent(&text.replace("{mcp_adapter}", adapter.to_str().unwrap()));
    sandbox.write(
        "project/.pi/agents/probe.md",
        &adapted(&host_pins::fixture("pi.md")),
    );
    if let Some(user) = user_agent {
        sandbox.write("pi-agent/agents/renamed.md", &adapted(user));
    }
    let shadows = host_pins::same_name_agents(&[
        sandbox.project().join(".pi/agents"),
        agent_dir.join("agents"),
    ]);
    let mut command = Command::new(&host.executable);
    command.args(["--no-extensions", "-e"]).arg(&adapter);
    command.arg("-e").arg(&subagents);
    command.args(["--approve", "--model", &format!("probe/{MODEL}")]);
    command.args(["-p", "Ask the probe agent what the glossary says."]);
    let ended = sandbox.run(
        command,
        &[
            ("PI_CODING_AGENT_DIR", agent_dir.as_os_str()),
            ("PI_OFFLINE", OsStr::new("1")),
        ],
    );
    assert_eq!(ended.code, Some(0), "{ended:?}");
    Session {
        requests: provider.requests(),
        shadows,
        stderr: ended.stderr,
    }
}

#[test]
#[ignore = "runs the installed Pi 0.87.1 and its installed providers in a sandbox; run explicitly"]
fn catalog_host_probe_pi_maps_tools_provider_model_and_skill_explicitly() {
    let session = session(&str::to_owned, None);
    let child = session.child();
    let mut tools = provider::offered_tools(child[0]);
    tools.sort();
    assert_eq!(
        tools,
        ["contact_supervisor", "maestro_knowledge_search", "read"]
    );
    let (parent_models, child_models) = session.models();
    assert!(!parent_models.is_empty() && !child_models.is_empty());
    assert!(
        parent_models.iter().all(|model| model == MODEL),
        "{parent_models:?}"
    );
    assert!(
        child_models.iter().all(|model| model == CHILD_MODEL),
        "{child_models:?}"
    );
    let system = provider::system_prompt(child[0]);
    assert!(system.contains("Marker: project-agent."), "{system}");
    assert!(
        system.contains("marker probe-skill-description"),
        "{system}"
    );
    let results = provider::tool_results(child.last().unwrap());
    assert!(results.contains("maestro-evidence/1"), "{results}");
    assert!(session.shadows.is_empty(), "{:?}", session.shadows);
    session.assert_no_agent_diagnostic();
    println!("pi projection: exact tools, explicit provider, model and skill; MCP call passed");
}

#[test]
#[ignore = "runs the installed Pi 0.87.1 and its installed providers in a sandbox; run explicitly"]
fn catalog_host_probe_pi_refuses_mcp_tools_without_their_provider() {
    let without_provider = |text: &str| {
        text.lines()
            .filter(|line| !line.starts_with("extensions:"))
            .fold(String::new(), |kept, line| kept + line + "\n")
    };
    let session = session(&without_provider, None);
    let offered = session
        .child()
        .iter()
        .flat_map(|request| provider::offered_tools(request))
        .collect::<Vec<_>>();
    assert!(
        !offered
            .iter()
            .any(|tool| tool == "maestro_knowledge_search"),
        "{offered:?}"
    );
    let parent = provider::tool_results(session.requests.last().unwrap());
    assert!(
        parent.contains("requested unavailable child tools: maestro_knowledge_search"),
        "{parent}"
    );
    assert!(!parent.contains("maestro-evidence/1"), "{parent}");
    println!("pi without its MCP provider: the child fails closed");
}

#[test]
#[ignore = "runs the installed Pi 0.87.1 and its installed providers in a sandbox; run explicitly"]
fn catalog_host_probe_pi_lets_the_project_agent_shadow_a_same_name_user_agent() {
    let user = host_pins::fixture("pi.md").replace("Marker: project-agent.", "Marker: user-agent.");
    let session = session(&str::to_owned, Some(&user));
    assert_eq!(session.shadows, ["probe"]);
    let system = provider::system_prompt(session.child()[0]);
    assert!(system.contains("Marker: project-agent."), "{system}");
    assert!(!system.contains("Marker: user-agent."), "{system}");
    session.assert_no_agent_diagnostic();
    println!("pi shadow: the project agent wins silently; the probe detects it");
}

#[test]
#[ignore = "runs the installed Pi 0.87.1 and its installed providers in a sandbox; run explicitly"]
fn catalog_host_probe_pi_refuses_a_child_model_it_does_not_know() {
    let absent_model = |text: &str| {
        text.replace(
            &format!("model: probe/{CHILD_MODEL}"),
            "model: probe/absent-model",
        )
    };
    let session = session(&absent_model, None);
    let (parent_models, child_models) = session.models();
    assert!(child_models.is_empty(), "the child ran on {child_models:?}");
    assert!(
        parent_models.iter().all(|model| model == MODEL),
        "{parent_models:?}"
    );
    let parent = provider::tool_results(session.requests.last().unwrap());
    let refusal = parent
        .lines()
        .find(|line| line.contains("absent-model"))
        .unwrap_or_else(|| panic!("{parent}"));
    println!("pi absent child model: the child does not run; no fallback model:\n  {refusal}");
}
