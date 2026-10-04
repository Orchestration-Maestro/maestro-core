//! G26: `setup`, `status` and `doctor` account for the local embedded graph
//! without a graph service, a port or a first-use download. Off by default,
//! the graph is never probed; a selected engine the build lacks is named;
//! a path from the caller is refused before anything opens; and health never
//! creates or changes a graph file.

use super::support::{Ended, Home};
#[cfg(feature = "engine")]
use maestro_kernel::artifact::Digest;
#[cfg(all(unix, feature = "engine"))]
use maestro_kernel::store::Database;
#[cfg(feature = "engine")]
use maestro_knowledge::graph::projection::EngineSettings;
use serde_json::Value;
use std::fs;
#[cfg(all(unix, feature = "engine"))]
use std::path::Path;
use std::path::PathBuf;
#[cfg(all(unix, feature = "engine"))]
use std::time::Instant;

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
pub(super) fn detail(ended: &Ended) -> String {
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

#[cfg(all(not(feature = "engine"), unix))]
#[test]
fn ladybug_in_a_build_without_the_engine_is_named_and_nothing_is_created() {
    let home = Home::bare();
    let status = home.run(&["--set", "graph.engine=ladybug", "--json", "status"]);
    assert_eq!(status.code, Some(0), "{status:?}");
    assert_eq!(graph(&status.json())["ready"], false);
    assert!(detail(&status).contains("built without the engine"));

    let doctor = home.run(&["--set", "graph.engine=ladybug", "--json", "doctor"]);
    assert_eq!(doctor.code, Some(1), "{doctor:?}");
    let next = graph(&doctor.json())["next_action"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(next.contains("`engine` feature"), "{next}");

    let (setup, calls, stderr) = setup_offline(
        &home,
        &["--set", "graph.engine=ladybug", "--json", "setup", "--yes"],
    );
    assert_eq!(setup["graph"]["action"], "refused", "{setup}");
    assert!(
        setup["graph"]["detail"]
            .as_str()
            .unwrap()
            .contains("`engine` feature")
    );
    assert!(stderr.contains("`engine` feature"), "{stderr}");
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        assert!(calls.contains("systemctl"), "Qdrant still ran: {calls}");
    } else {
        assert!(
            stderr.contains("set Qdrant") && stderr.contains("up by hand"),
            "Qdrant still reported its manual setup: {stderr}"
        );
        assert!(calls.is_empty(), "manual setup runs no tools: {calls}");
    }
    assert!(!home.data().join("graph").exists());
    assert!(
        !home.data().join("qdrant").exists(),
        "the service refusal remains unchanged"
    );
}

#[cfg(not(feature = "engine"))]
#[test]
fn runtime_admission_still_refuses_the_uncompiled_graph_backend() {
    let home = Home::bare();
    let runtime = home.run(&["--set", "graph.engine=ladybug", "knowledge", "collections"]);
    assert_eq!(runtime.code, Some(2), "{runtime:?}");
    assert_eq!(
        runtime.stderr.trim(),
        "backend adapter is not compiled into this build"
    );
    assert!(!home.data().join("graph").exists());
}

#[cfg(unix)]
#[test]
fn setup_refuses_a_settings_file_error_and_still_reports_qdrant() {
    let home = Home::bare();
    fs::write(home.config().join("preferences.toml"), "schema = [\n").unwrap();
    let setup = home.run(&["setup"]);
    assert_eq!(setup.code, Some(2), "{setup:?}");
    assert!(setup.stdout.starts_with("Graph: refused:"), "{setup:?}");
    assert!(setup.stderr.contains("preferences.toml"), "{setup:?}");
    assert!(!home.data().join("qdrant").exists());
}

#[test]
fn lbug_is_refused_with_an_explicit_migration_before_any_graph_call() {
    let home = Home::bare();
    for command in ["status", "doctor", "setup"] {
        let refused = home.run(&["--set", "graph.engine=lbug", command]);
        assert_eq!(refused.code, Some(2), "{command}: {refused:?}");
        assert!(
            refused
                .stderr
                .contains("replace graph.engine=lbug with graph.engine=ladybug"),
            "{refused:?}"
        );
    }
    assert!(!home.data().join("graph").exists());
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
fn setup_offline(home: &Home, arguments: &[&str]) -> (Value, String, String) {
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
    (
        document,
        fs::read_to_string(calls).unwrap_or_default(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[cfg(unix)]
#[test]
fn an_off_graph_setup_downloads_nothing_and_writes_nothing() {
    let home = Home::bare();
    let (document, calls, _) = setup_offline(&home, &["--json", "setup", "--yes"]);
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
    let missing = home.run(&["--set", "graph.engine=ladybug", "--json", "doctor"]);
    assert_eq!(detail(&missing), "the graph directory is missing");

    let lbug = ["--set", "graph.engine=ladybug", "--json", "setup"];
    let (preview, _, _) = setup_offline(&home, &lbug);
    assert_eq!(preview["graph"]["action"], "create_directory", "{preview}");
    assert_eq!(
        preview["graph"]["missing_guards"],
        serde_json::json!([".access.guard", ".writer.guard"])
    );
    assert!(!graph_directory.exists());
    let started = Instant::now();
    let (applied, calls, _) = setup_offline(&home, &[&lbug[..], &["--yes"]].concat());
    println!("G26_SETUP_INSTALL_US={}", started.elapsed().as_micros());
    assert_eq!(applied["graph"]["changed"], true, "{applied}");
    assert!(graph_directory.is_dir());
    assert!(!calls.contains("curl") && !calls.contains("tar"), "{calls}");
    let mode = fs::metadata(&graph_directory).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
    assert_permanent_guards(&graph_directory);

    fs::write(graph_directory.join("unpublished.lbug"), "kept").unwrap();
    assert_missing_graph_authority(&home);
    drop(Database::open_in(&home.data()).unwrap());
    let doctor = home.run(&["--set", "graph.engine=ladybug", "--json", "doctor"]);
    assert!(detail(&doctor).starts_with("no graph is published yet, so no file was opened"));
    assert!(detail(&doctor).contains("no admitted authoring lock"));
    assert_eq!(graph(&doctor.json())["checked"], false);
    assert_eq!(
        fs::read_to_string(graph_directory.join("unpublished.lbug")).unwrap(),
        "kept"
    );

    fs::set_permissions(&graph_directory, fs::Permissions::from_mode(0o755)).unwrap();
    let shared = home.run(&["--set", "graph.engine=ladybug", "--json", "status"]);
    assert!(detail(&shared).contains("permissions"), "{shared:?}");
    let mode = fs::metadata(&graph_directory).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755, "status changes nothing");
}

/// The permanent guards are real private regular files, not engine sidecars.
#[cfg(all(unix, feature = "engine"))]
fn assert_permanent_guards(directory: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    for name in [".access.guard", ".writer.guard"] {
        let guard = directory.join(name);
        assert!(guard.is_file());
        assert_eq!(
            fs::metadata(guard).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[cfg(all(unix, feature = "engine"))]
#[test]
fn graph_health_status_and_doctor_report_permanent_guard_contention() {
    use maestro_filesystem::{ControlFile, LockMode, OwnedRoot, SystemFileLock};
    let home = Home::bare();
    let directory = home.data().join("graph");
    let root = OwnedRoot::open(&directory, true).unwrap();
    for control in [ControlFile::Access, ControlFile::Writer] {
        root.ensure_control(control).unwrap();
    }
    for (control, diagnosis) in [
        (ControlFile::Access, "cleanup in progress"),
        (ControlFile::Writer, "a writer holds the graph file's lock"),
    ] {
        let held = root.open_control(control).unwrap();
        held.lock_with(&SystemFileLock, LockMode::Exclusive, false)
            .unwrap();
        for command in ["status", "doctor"] {
            let report = home.run(&["--set", "graph.engine=ladybug", "--json", command]);
            assert_eq!(detail(&report), diagnosis);
            let text = home.run(&["--set", "graph.engine=ladybug", command]);
            assert!(text.stdout.contains(diagnosis), "{text:?}");
            if command == "doctor" {
                assert!(
                    graph(&report.json())["next_action"]
                        .as_str()
                        .unwrap()
                        .contains("finish")
                );
            } else {
                assert_eq!(graph(&report.json())["ready"], false);
            }
        }
    }
    fs::remove_file(directory.join(ControlFile::Writer.file_name())).unwrap();
    let report = home.run(&["--set", "graph.engine=ladybug", "--json", "doctor"]);
    assert_eq!(
        detail(&report),
        "a permanent graph guard is missing or unsafe"
    );
    assert!(
        !directory.join(ControlFile::Writer.file_name()).exists(),
        "health never repairs controls"
    );
}

/// Missing authority is not the same as an empty inventory.
#[cfg(all(unix, feature = "engine"))]
fn assert_missing_graph_authority(home: &Home) {
    let unavailable = home.run(&["--set", "graph.engine=ladybug", "--json", "doctor"]);
    assert_eq!(detail(&unavailable), "the kernel authority is missing");
    assert_eq!(graph(&unavailable.json())["checked"], true);
    assert!(
        graph(&unavailable.json())["next_action"]
            .as_str()
            .unwrap()
            .contains("collection add")
    );
}

#[cfg(feature = "engine")]
#[test]
fn graph_operations_real_lock_enables_read_only_corruption_and_lock_diagnoses() {
    use maestro_filesystem::{ControlFile, LockMode, OwnedRoot, SystemFileLock};
    let home = Home::new();
    let workspace = admitted_workspace(
        &home,
        concat!(
            "schema = 'maestro-preferences/1'\n[overrides]\n",
            "'graphdb.buffer_pool_size' = 16777216\n'graphdb.max_db_size' = 67108864\n",
            "'graphdb.max_num_threads' = 1\n"
        ),
    );
    let lock_path = workspace.join(".maestro/authoring.lock.json");
    let lock_before = fs::read(&lock_path).unwrap();
    let settings = EngineSettings::new(
        16 * 1024 * 1024,
        64 * 1024 * 1024,
        1,
        Digest::of(&lock_before),
    )
    .unwrap();
    let (_, name) = super::graph_cleanup_support::published(&home, &settings);
    let directory = home.data().join("graph");
    let path = directory.join(name);
    let before = fs::read(&path).unwrap();
    let no_lock = home.run(&["--set", "graph.engine=ladybug", "--json", "doctor"]);
    assert!(
        detail(&no_lock).contains("no admitted authoring lock"),
        "{no_lock:?}"
    );
    assert_eq!(fs::read(&path).unwrap(), before);

    for command in ["status", "doctor"] {
        let report = home.run_in(
            &workspace,
            &["--set", "graph.engine=ladybug", "--json", command],
        );
        assert!(
            detail(&report).contains("corrupt or unreadable"),
            "{report:?}"
        );
        assert!(!detail(&report).contains("not activated"));
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::read(&lock_path).unwrap(), lock_before);
    }
    let root = OwnedRoot::open(&directory, false).unwrap();
    let writer = root.open_control(ControlFile::Writer).unwrap();
    writer
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    let locked = home.run_in(
        &workspace,
        &["--set", "graph.engine=ladybug", "--json", "doctor"],
    );
    assert_eq!(detail(&locked), "a writer holds the graph file's lock");
    assert_eq!(fs::read(&path).unwrap(), before);
    drop(writer);
    fs::write(&lock_path, [lock_before.as_slice(), b"\n"].concat()).unwrap();
    let changed = fs::read(&lock_path).unwrap();
    let refused = home.run_in(
        &workspace,
        &["--set", "graph.engine=ladybug", "--json", "doctor"],
    );
    assert!(
        detail(&refused).contains("authoring.lock.json"),
        "{refused:?}"
    );
    assert!(
        !detail(&refused).contains("corrupt"),
        "no native open without activation"
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fs::read(&lock_path).unwrap(), changed);
    let runtime = home.run_in(&workspace, &["knowledge", "collections"]);
    assert_eq!(runtime.code, Some(2), "{runtime:?}");
}

/// A real C04-owned lock, approved through the user-local journal, not lock self-authority.
pub(super) fn admitted_workspace(home: &Home, defaults: &str) -> PathBuf {
    use maestro_catalog::{
        files::{self, FileInput, FilePlan},
        policy::workspace::{CheckedTrust, JournalTrust, TrustBoundaries},
    };
    let workspace = home.root().join("project");
    fs::create_dir(&workspace).unwrap();
    let workspace = workspace.canonicalize().unwrap();
    let text = workspace.to_str().unwrap();
    let approved = home.run(&["trust", "add", text, "--confirm-path", text]);
    assert_eq!(approved.code, Some(0), "{approved:?}");
    let kernel = home.database();
    let adapter = JournalTrust::new(&kernel);
    let boundaries = TrustBoundaries::new(home.root(), &[home.data(), home.config()]).unwrap();
    let trust = CheckedTrust::new(&adapter, &boundaries);
    let bytes = serde_json::to_vec(&serde_json::json!({
        "schema": "maestro-authoring-lock/3", "defaults": defaults,
        "backend_types": ["ladybug"], "files": [],
        "sources": [{"path":"core/backends/graphdb/config.toml","sha256":"synthetic"}]
    }))
    .unwrap();
    let plan = FilePlan::preview(
        &workspace,
        [FileInput::new(".maestro/authoring.lock.json", bytes)],
        &trust,
    )
    .unwrap();
    files::apply(&workspace, &plan, &trust).unwrap();
    workspace
}

#[cfg(not(feature = "engine"))]
#[test]
fn graph_operations_lock_selected_uncompiled_engine_is_named_without_writes() {
    let home = Home::bare();
    let workspace = admitted_workspace(
        &home,
        "schema = 'maestro-preferences/1'\n[overrides]\n'graph.engine' = 'ladybug'\n",
    );
    let lock = workspace.join(".maestro/authoring.lock.json");
    let before = fs::read(&lock).unwrap();
    for command in ["status", "doctor"] {
        let report = home.run_in(&workspace, &["--json", command]);
        assert_eq!(
            detail(&report),
            "graph.engine = ladybug, but this maestro was built without the engine",
            "{report:?}"
        );
        assert_eq!(report.code, Some(i32::from(command != "status")));
    }
    assert_eq!(fs::read(lock).unwrap(), before);
    assert!(!home.data().join("graph").exists());
    let runtime = home.run_in(&workspace, &["knowledge", "collections"]);
    assert_eq!(runtime.code, Some(2));
    assert!(runtime.stderr.contains("not compiled"));
}
