//! G26: `setup`, `status` and `doctor` account for the local embedded graph
//! without a graph service, a port or a first-use download. Off by default,
//! the graph is never probed; a selected engine the build lacks is named;
//! a path from the caller is refused before anything opens; and health never
//! creates or changes a graph file.

use super::support::{Ended, Home};
use serde_json::Value;
#[cfg(unix)]
use std::fs;

/// The graph's entry of a `status` or `doctor` document.
fn graph(document: &Value) -> &Value {
    ["checks", "services"]
        .iter()
        .filter_map(|key| document[*key].as_array())
        .flatten()
        .find(|check| check["name"] == "graph")
        .expect("a graph entry")
}

/// The graph's detail in the document `ended` printed.
fn detail(ended: &Ended) -> String {
    graph(&ended.json())["detail"]
        .as_str()
        .expect("a detail")
        .to_owned()
}

#[test]
fn an_off_graph_is_reported_off_and_never_probed_or_created() {
    let home = Home::bare();
    let status = home.run(&["--json", "status"]);
    assert_eq!(status.code, Some(0), "{status:?}");
    assert_eq!(detail(&status), "the graph is off (graph.engine = none)");
    assert_eq!(graph(&status.json())["ready"], true);
    assert_eq!(
        graph(&status.json())["target"],
        home.data().join("graph").display().to_string()
    );

    let doctor = home.run(&["--json", "doctor"]);
    assert_eq!(detail(&doctor), "the graph is off (graph.engine = none)");
    assert_eq!(graph(&doctor.json())["checked"], false);
    assert!(graph(&doctor.json())["next_action"].is_null());
    assert!(
        !home.data().join("graph").exists(),
        "health creates nothing"
    );
}

#[cfg(not(feature = "engine"))]
#[test]
fn lbug_in_a_build_without_the_engine_is_named_and_nothing_is_created() {
    let home = Home::bare();
    let status = home.run(&["--set", "graph.engine=lbug", "--json", "status"]);
    assert_eq!(status.code, Some(0), "{status:?}");
    assert_eq!(graph(&status.json())["ready"], false);
    assert!(detail(&status).contains("built without the engine"));

    let doctor = home.run(&["--set", "graph.engine=lbug", "--json", "doctor"]);
    assert_eq!(doctor.code, Some(1), "{doctor:?}");
    let next = graph(&doctor.json())["next_action"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(next.contains("`engine` feature"), "{next}");

    let setup = home.run(&["--set", "graph.engine=lbug", "setup", "--yes"]);
    assert_eq!(setup.code, Some(2), "{setup:?}");
    assert_eq!(setup.stdout, "");
    assert!(setup.stderr.contains("`engine` feature"), "{setup:?}");
    assert!(!home.data().join("graph").exists());
    assert!(
        !home.data().join("qdrant").exists(),
        "the service untouched"
    );
}

#[test]
fn an_engine_path_from_the_caller_is_refused_before_anything_opens() {
    let home = Home::bare();
    for (flag, refusal) in [
        ("graph.path=/elsewhere/graph.lbug", "unknown key"),
        ("graph.engine=/elsewhere/graph.lbug", "graph.engine"),
    ] {
        for command in ["status", "doctor", "setup"] {
            let refused = home.run(&["--set", flag, command]);
            assert_eq!(refused.code, Some(2), "{flag} {command}: {refused:?}");
            assert!(refused.stderr.contains(refusal), "{refused:?}");
        }
    }
    assert!(!home.data().join("graph").exists());
}

/// Runs `setup` with `arguments` in `home` where every tool it could call,
/// `curl`, `tar` and `systemctl`, is a fake that fails, and records its
/// calls: no systemd user manager runs, so the search service's part is
/// refused. Returns what it printed and the tools it called.
#[cfg(unix)]
fn setup_offline(home: &Home, arguments: &[&str]) -> (Value, String) {
    use std::{env, os::unix::fs::PermissionsExt as _};
    let tools = home.tools();
    let calls = tools.join("calls");
    let script = format!(
        "#!/bin/sh\necho \"$0 $*\" >> '{}'\nexit 1\n",
        calls.display()
    );
    for tool in ["curl", "tar", "systemctl"] {
        let path = tools.join(tool);
        fs::write(&path, &script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let path = format!("{}:{}", tools.display(), env::var("PATH").unwrap());
    let mut command = home.command(arguments);
    command.env("PATH", path);
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let document = serde_json::from_slice(&output.stdout).unwrap();
    (document, fs::read_to_string(calls).unwrap_or_default())
}

#[cfg(unix)]
#[test]
fn an_off_graph_setup_downloads_nothing_and_writes_nothing() {
    let home = Home::bare();
    let (document, calls) = setup_offline(&home, &["--json", "setup", "--yes"]);
    assert_eq!(document["graph"]["action"], "disabled", "{document}");
    assert!(!home.data().join("graph").exists());
    assert!(
        !calls.contains("curl") && !calls.contains("tar"),
        "no download: {calls}"
    );
}

#[cfg(all(unix, feature = "engine"))]
#[test]
fn with_the_engine_setup_owns_the_directory_and_health_opens_nothing_yet() {
    use std::os::unix::fs::PermissionsExt as _;
    let home = Home::bare();
    let graph_directory = home.data().join("graph");
    let missing = home.run(&["--set", "graph.engine=lbug", "--json", "doctor"]);
    assert_eq!(detail(&missing), "the graph directory is missing");

    let lbug = ["--set", "graph.engine=lbug", "--json", "setup"];
    let (preview, _) = setup_offline(&home, &lbug);
    assert_eq!(preview["graph"]["action"], "create_directory", "{preview}");
    assert!(!graph_directory.exists());
    let (applied, calls) = setup_offline(&home, &[&lbug[..], &["--yes"]].concat());
    assert_eq!(applied["graph"]["changed"], true, "{applied}");
    assert!(graph_directory.is_dir());
    assert!(!calls.contains("curl") && !calls.contains("tar"), "{calls}");
    let mode = fs::metadata(&graph_directory).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);

    fs::write(graph_directory.join("unpublished.lbug"), "kept").unwrap();
    let doctor = home.run(&["--set", "graph.engine=lbug", "--json", "doctor"]);
    assert_eq!(
        detail(&doctor),
        "no graph is published yet, so no file was opened"
    );
    assert_eq!(graph(&doctor.json())["checked"], false);
    assert_eq!(
        fs::read_to_string(graph_directory.join("unpublished.lbug")).unwrap(),
        "kept"
    );

    fs::set_permissions(&graph_directory, fs::Permissions::from_mode(0o755)).unwrap();
    let shared = home.run(&["--set", "graph.engine=lbug", "--json", "status"]);
    assert!(detail(&shared).contains("permissions"), "{shared:?}");
    let mode = fs::metadata(&graph_directory).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755, "status changes nothing");
}
