//! Review regressions for lock-only graph selection and preference precedence.

#[cfg(not(feature = "engine"))]
use super::support::make_safe_preferences_path;
use super::{
    graph_operations::{admitted_workspace, detail},
    support::Home,
};
use std::fs;

/// Synthetic defaults selecting the engine through an owned authoring lock only.
const LADYBUG: &str = "schema = 'maestro-preferences/1'\n[overrides]\n'graph.engine' = 'ladybug'\n";
/// Explicit preferences disabling graph reads.
#[cfg(not(feature = "engine"))]
const NONE: &str = "schema = 'maestro-preferences/1'\n[overrides]\n'graph.engine' = 'none'\n";

#[cfg(not(feature = "engine"))]
#[test]
fn graph_operations_lock_none_override_stays_off() {
    for source in ["flag", "project", "user"] {
        let home = Home::bare();
        let workspace = admitted_workspace(&home, LADYBUG);
        let lock = workspace.join(".maestro/authoring.lock.json");
        let before = fs::read(&lock).unwrap();
        let preferences = match source {
            "project" => Some(workspace.join(".maestro/config.toml")),
            "user" => Some(home.config().join("preferences.toml")),
            _ => None,
        };
        if let Some(path) = &preferences {
            fs::write(path, NONE).unwrap();
            make_safe_preferences_path(path);
            make_safe_preferences_path(path.parent().unwrap());
        }
        for command in ["status", "doctor"] {
            let arguments = if source == "flag" {
                vec!["--set", "graph.engine=none", "--json", command]
            } else {
                vec!["--json", command]
            };
            let report = home.run_in(&workspace, &arguments);
            assert_eq!(
                detail(&report),
                "the graph is off (graph.engine = none)",
                "{source}: {report:?}"
            );
        }
        assert_eq!(fs::read(lock).unwrap(), before);
        if let Some(path) = preferences {
            assert_eq!(fs::read_to_string(path).unwrap(), NONE);
        }
        assert!(!home.data().join("graph").exists());
    }
}

#[test]
fn graph_operations_lock_only_refusal_is_named() {
    let home = Home::bare();
    let workspace = admitted_workspace(&home, LADYBUG);
    let lock = workspace.join(".maestro/authoring.lock.json");
    let changed = [fs::read(&lock).unwrap().as_slice(), b"\n"].concat();
    fs::write(&lock, &changed).unwrap();
    for command in ["status", "doctor"] {
        let report = home.run_in(&workspace, &["--json", command]);
        assert!(
            detail(&report).contains("authoring.lock.json"),
            "{report:?}"
        );
        assert!(!detail(&report).contains("graph is off"), "{report:?}");
        assert!(report.stdout.contains("maestro init"), "{report:?}");
        assert_eq!(report.code, Some(i32::from(command != "status")));
    }
    let runtime = home.run_in(&workspace, &["knowledge", "collections"]);
    assert_eq!(runtime.code, Some(2), "{runtime:?}");
    assert!(
        runtime.stderr.contains("authoring.lock.json"),
        "{runtime:?}"
    );
    assert_eq!(fs::read(lock).unwrap(), changed);
    assert!(!home.data().join("graph").exists());
}
