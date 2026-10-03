//! The pinned hosts of owner approval OA2: a host that is missing or reports
//! another version, checked inside a sandbox, blocks its probe.

use super::{
    code_digests::{self, Code, copilot_platform_binary},
    host_sandbox::Sandbox,
};
use crate::{
    knowledge_get::cli_cases::{SET_ID, published_glossary},
    support::Home,
};
use serde_json::{Value, json};
use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// A host approved by OA2 at one exact version: its command, the first line
/// its version command prints, and the code its digest covers.
pub(super) struct Pin {
    /// The command name looked up on `PATH`.
    pub(super) command: &'static str,
    /// The arguments that print the version.
    pub(super) arguments: &'static [&'static str],
    /// The exact first line of the version output.
    pub(super) version: &'static str,
    /// The code that runs, which the printed digest covers.
    pub(super) code: Code,
}

/// GitHub Copilot CLI 1.0.88.
pub(super) const COPILOT: Pin = Pin {
    command: "copilot",
    arguments: &["--version"],
    version: "GitHub Copilot CLI 1.0.88.",
    code: Code::PackageFile(copilot_platform_binary),
};

/// Pi 0.87.1.
pub(super) const PI: Pin = Pin {
    command: "pi",
    arguments: &["--version"],
    version: "0.87.1",
    code: Code::PackageTree,
};

/// Claude Code 2.1.283.
pub(super) const CLAUDE_CODE: Pin = Pin {
    command: "claude",
    arguments: &["--version"],
    version: "2.1.283 (Claude Code)",
    code: Code::Executable,
};

/// codex-cli 0.150.1.
pub(super) const CODEX: Pin = Pin {
    command: "codex",
    arguments: &["--version"],
    version: "codex-cli 0.150.1",
    code: Code::Executable,
};

/// Why a probe cannot run: a missing host, version or provider. A blocked
/// probe is never a pass.
#[derive(Debug)]
pub(super) struct Blocked(pub(super) String);

/// A pinned host found on `PATH`: its executable, with symbolic links
/// resolved. [`resolve`] prints the digest of the code it runs.
pub(super) struct Host {
    /// The resolved executable.
    pub(super) executable: PathBuf,
}

/// Finds `pin` on `path` and checks its version in an empty sandbox.
pub(super) fn resolve(pin: &Pin, path: &OsStr) -> Result<Host, Blocked> {
    let executable = env::split_paths(path)
        .flat_map(|directory| {
            [
                directory.join(pin.command),
                directory.join(format!("{}{}", pin.command, env::consts::EXE_SUFFIX)),
            ]
        })
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| Blocked(format!("{} is not on PATH", pin.command)))?;
    let sandbox = Sandbox::new();
    let mut command = Command::new(&executable);
    command.args(pin.arguments);
    let ended = sandbox.run_with_path(command, path);
    let first = ended.stdout.lines().next().unwrap_or_default().trim();
    if first != pin.version {
        return Err(Blocked(format!(
            "{} reports {first:?}, not the pinned {:?}",
            pin.command, pin.version
        )));
    }
    let executable = fs::canonicalize(&executable).unwrap();
    let (code, digest) = code_digests::covered(&pin.code, &executable);
    println!(
        "pin {}: {} runs {} sha256:{}",
        pin.version,
        executable.display(),
        code.display(),
        digest.as_str()
    );
    Ok(Host { executable })
}

/// Finds `pin` on this process's `PATH`; a blocked probe fails its test.
pub(super) fn require(pin: &Pin) -> Host {
    resolve(pin, &env::var_os("PATH").unwrap_or_default())
        .unwrap_or_else(|blocked| panic!("blocked: {}", blocked.0))
}

/// An installed Pi package at its approved version: the directory holding
/// its `package.json`.
pub(super) fn pi_package(packages: &Path, name: &str, version: &str) -> Result<PathBuf, Blocked> {
    let directory = packages.join(name);
    let manifest = fs::read_to_string(directory.join("package.json"))
        .map_err(|_| Blocked(format!("{name} is not installed in {}", packages.display())))?;
    let found = serde_json::from_str::<Value>(&manifest)
        .ok()
        .and_then(|manifest| manifest["version"].as_str().map(str::to_owned))
        .unwrap_or_default();
    if found != version {
        return Err(Blocked(format!(
            "{name} is {found:?}, not the pinned {version:?}"
        )));
    }
    Ok(directory)
}

/// A Maestro home holding the published synthetic glossary, served by the
/// built `maestro mcp`.
pub(super) struct Knowledge(Home);

impl Knowledge {
    /// A home whose `synthetic` collection has a published generation.
    pub(super) fn published() -> Self {
        let home = Home::new();
        published_glossary(&home, SET_ID, (0, None));
        Self(home)
    }

    /// The `maestro` MCP server entry of an `mcpServers` map, with `extra`
    /// host-specific keys merged in.
    pub(super) fn server(&self, extra: &Value) -> Value {
        let mut server = json!({
            "type": "stdio",
            "command": env!("CARGO_BIN_EXE_maestro"),
            "args": ["mcp"],
            "env": {
                "XDG_DATA_HOME": self.0.root().join("data"),
                "XDG_CONFIG_HOME": self.0.root().join("config"),
            },
        });
        for (key, value) in extra.as_object().unwrap() {
            server[key] = value.clone();
        }
        server
    }
}

/// A catalog fixture of `tests/fixtures/catalog/hosts`.
pub(super) fn fixture(name: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    fs::read_to_string(root.join("tests/fixtures/catalog/hosts").join(name)).unwrap()
}

/// The `name:` of every agent profile directly in `directories` that more
/// than one profile declares: the same-name shadows a projection must report.
pub(super) fn same_name_agents(directories: &[PathBuf]) -> Vec<String> {
    let mut names = directories
        .iter()
        .filter_map(|directory| fs::read_dir(directory).ok())
        .flatten()
        .filter_map(|entry| fs::read_to_string(entry.ok()?.path()).ok())
        .filter_map(|text| {
            let front = text
                .strip_prefix("---\n")?
                .split("\n---")
                .next()?
                .to_owned();
            front.lines().find_map(|line| {
                line.strip_prefix("name:")
                    .map(|name| name.trim().to_owned())
            })
        })
        .collect::<Vec<_>>();
    names.sort();
    let mut shadows = names
        .windows(2)
        .filter(|pair| pair[0] == pair[1])
        .map(|pair| pair[0].clone())
        .collect::<Vec<_>>();
    shadows.dedup();
    shadows
}

#[test]
fn a_host_missing_from_path_blocks_the_probe() {
    let empty = Sandbox::new();
    let blocked = resolve(&COPILOT, empty.home().as_os_str()).err().unwrap();
    assert_eq!(blocked.0, "copilot is not on PATH");
}

#[test]
fn a_host_at_another_version_blocks_the_probe() {
    let maestro = Path::new(env!("CARGO_BIN_EXE_maestro"));
    let other = Pin {
        command: "maestro",
        arguments: &["--version"],
        version: "maestro 9.9.9",
        code: Code::Executable,
    };
    let blocked = resolve(&other, maestro.parent().unwrap().as_os_str())
        .err()
        .unwrap();
    assert!(
        blocked.0.starts_with("maestro reports \"maestro "),
        "{blocked:?}"
    );
    assert!(
        blocked.0.ends_with("not the pinned \"maestro 9.9.9\""),
        "{blocked:?}"
    );
}

#[test]
fn a_missing_or_other_pi_provider_blocks_the_probe() {
    let packages = Sandbox::new();
    let missing = pi_package(&packages.home(), "pi-mcp-adapter", "2.37.0")
        .err()
        .unwrap();
    assert!(
        missing.0.starts_with("pi-mcp-adapter is not installed in "),
        "{missing:?}"
    );
    packages.write(
        "home/pi-mcp-adapter/package.json",
        r#"{"version": "2.36.0"}"#,
    );
    let other = pi_package(&packages.home(), "pi-mcp-adapter", "2.37.0")
        .err()
        .unwrap();
    assert_eq!(
        other.0,
        r#"pi-mcp-adapter is "2.36.0", not the pinned "2.37.0""#
    );
}

#[test]
fn same_name_agents_across_roots_are_reported_once() {
    let roots = Sandbox::new();
    let profile = |name: &str| format!("---\nname: {name}\ndescription: d\n---\nbody\n");
    roots.write("home/one.agent.md", &profile("probe"));
    roots.write("home/two.agent.md", &profile("other"));
    roots.write("project/three.agent.md", &profile("probe"));
    roots.write("project/four.md", "no front matter\n");
    assert_eq!(
        same_name_agents(&[roots.home(), roots.project()]),
        ["probe"]
    );
    assert!(same_name_agents(&[roots.home()]).is_empty());
}
