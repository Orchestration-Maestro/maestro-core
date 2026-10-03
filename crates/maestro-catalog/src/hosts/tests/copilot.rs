//! Fixture-backed native projection; never a live host-obedience receipt.
use super::super::copilot::NativeProjection as _;
use super::super::{Copilot, SourceSnapshot};
use crate::{
    bootstrap::AreaInventories,
    files::tests::support::with_trust,
    source::{Known, builtin, frozen_rows},
};
use maestro_test_scratch::scratch_directory;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

struct Fixture {
    root: PathBuf,
    project: PathBuf,
    catalog: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = scratch_directory().unwrap();
        let project = root.join("project");
        let catalog = root.join("catalog");
        fs::create_dir(&project).unwrap();
        let original = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/catalog/bootstrap/owner-local");
        copy_tree(&original, &catalog);
        Self {
            root,
            project,
            catalog,
        }
    }
    fn port(&self) -> Result<AreaInventories, String> {
        let rows = frozen_rows();
        let settings = maestro_settings::Registry::built_in().unwrap();
        AreaInventories::new(
            &self.catalog,
            builtin().unwrap(),
            Known {
                rows: &rows,
                settings: &settings,
                today: 0,
            },
        )
    }
    fn snapshot(&self) -> SourceSnapshot {
        SourceSnapshot::resolve(&self.port().unwrap(), &["base".to_owned()]).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn hosts_copilot_fixture_preview_apply_drift_and_owned_only_remove() {
    let fixture = Fixture::new();
    let snapshot = fixture.snapshot();
    let adapter = Copilot::new(None);
    with_trust(&fixture.project, |trust| {
        let neighbour = json!({"mcpServers":{"user":{"command":"user"}}});
        fs::write(fixture.project.join(".mcp.json"), neighbour.to_string()).unwrap();
        let preview = adapter
            .preview(&fixture.project, &snapshot, false, trust)
            .unwrap();
        assert!(!fixture.project.join(".github").exists());
        assert!(!preview.registered);
        assert!(!preview.observed);
        assert!(
            preview
                .diagnosis
                .contains("reload inside a running session: not run")
        );
        preview.apply(&fixture.project, trust).unwrap();
        let replay = adapter
            .preview(&fixture.project, &snapshot, false, trust)
            .unwrap();
        assert!(replay.registered);
        replay.apply(&fixture.project, trust).unwrap();
        assert_qualified_agent(&fixture);
        let original = fs::read(fixture.project.join(".mcp.json")).unwrap();
        let mut edited: Value = serde_json::from_slice(&original).unwrap();
        edited["mcpServers"]["maestro"]["command"] = json!("edited");
        fs::write(fixture.project.join(".mcp.json"), edited.to_string()).unwrap();
        assert!(
            adapter
                .preview(&fixture.project, &snapshot, false, trust)
                .is_err()
        );
        assert!(
            adapter
                .preview(&fixture.project, &snapshot, true, trust)
                .is_err()
        );
        let mut valid: Value = serde_json::from_slice(&original).unwrap();
        valid["neighbour"] = json!(42);
        fs::write(fixture.project.join(".mcp.json"), valid.to_string()).unwrap();
        adapter
            .preview(&fixture.project, &snapshot, true, trust)
            .unwrap()
            .apply(&fixture.project, trust)
            .unwrap();
        assert!(
            !fixture
                .project
                .join(".github/agents/maestro.agent.md")
                .exists()
        );
        let remaining: Value =
            serde_json::from_slice(&fs::read(fixture.project.join(".mcp.json")).unwrap()).unwrap();
        assert_eq!(remaining["mcpServers"]["user"]["command"], "user");
        assert_eq!(remaining["neighbour"], 42);
        assert!(remaining["mcpServers"].get("maestro").is_none());
        adapter
            .preview(&fixture.project, &snapshot, true, trust)
            .unwrap()
            .apply(&fixture.project, trust)
            .unwrap();
    });
}

/// Check the entire qualified body independently of the production frontmatter splitter.
fn assert_qualified_agent(fixture: &Fixture) {
    let agent =
        fs::read_to_string(fixture.project.join(".github/agents/maestro.agent.md")).unwrap();
    assert!(agent.contains("tools: [\"maestro/knowledge_search\", \"view\"]"));
    assert!(!agent.contains("metadata:"));
    let qualified =
        fs::read_to_string(fixture.catalog.join("core/agents/maestro.agent.md")).unwrap();
    let body = qualified.split_once("\n---\n").unwrap().1;
    assert!(agent.contains(body));
    assert!(agent.contains("Follow the current MCP session's conversation language and tone."));
}

#[test]
fn hosts_copilot_source_admission_never_drops_unqualified_models_or_tools() {
    let fixture = Fixture::new();
    let profile = fixture.catalog.join("core/agents/maestro.agent.md");
    let original = fs::read_to_string(&profile).unwrap();
    fixture.snapshot();
    for (field, diagnosis) in [
        (
            "model: unavailable\n",
            "model mapping not qualified for Copilot (C01)",
        ),
        (
            "mcp-servers: []\n",
            "model mapping not qualified for Copilot (C01)",
        ),
        (
            "tools: [view, unavailable]\n",
            "tool `unavailable` not qualified for Copilot",
        ),
    ] {
        let changed = original.replace("tools: [\"view\"]\n", field);
        assert_ne!(changed, original);
        fs::write(&profile, changed).unwrap();
        let error =
            SourceSnapshot::resolve(&fixture.port().unwrap(), &["base".to_owned()]).unwrap_err();
        assert!(error.contains("core/agents/maestro.agent.md"), "{error}");
        assert!(error.contains(diagnosis), "{error}");
    }
    fs::write(profile, original).unwrap();
    fixture.snapshot();
}

#[test]
fn hosts_copilot_preview_is_bound_to_exact_target_even_inside_trust() {
    let fixture = Fixture::new();
    let snapshot = fixture.snapshot();
    with_trust(&fixture.project, |trust| {
        let plan = Copilot::new(None)
            .preview(&fixture.project, &snapshot, false, trust)
            .unwrap();
        let other = fixture.project.join("other");
        fs::create_dir(&other).unwrap();
        assert!(plan.apply(&other, trust).is_err());
        assert_eq!(fs::read_dir(other).unwrap().count(), 0);
        plan.apply(&fixture.project, trust).unwrap();
    });
}

#[test]
fn hosts_copilot_refuses_source_drift_and_target_drift() {
    let fixture = Fixture::new();
    let snapshot = fixture.snapshot();
    let adapter = Copilot::new(None);
    with_trust(&fixture.project, |trust| {
        let plan = adapter
            .preview(&fixture.project, &snapshot, false, trust)
            .unwrap();
        fs::write(fixture.project.join(".mcp.json"), b"{}").unwrap();
        assert!(plan.apply(&fixture.project, trust).is_err());
        assert!(!fixture.project.join(".github").exists());
        fs::remove_file(fixture.project.join(".mcp.json")).unwrap();
        let plan = adapter
            .preview(&fixture.project, &snapshot, false, trust)
            .unwrap();
        fs::write(
            fixture.catalog.join("core/agents/maestro.agent.md"),
            b"edited",
        )
        .unwrap();
        assert!(plan.apply(&fixture.project, trust).is_err());
        assert!(!fixture.project.join(".github").exists());
    });
}

#[test]
fn hosts_copilot_declared_name_not_stem_detects_user_and_project_shadows() {
    let fixture = Fixture::new();
    let snapshot = fixture.snapshot();
    let user = fixture.root.join("user-agents");
    fs::create_dir(&user).unwrap();
    let adapter = Copilot::new(Some(user.clone()));
    with_trust(&fixture.project, |trust| {
        let valid = "---\nname: neighbour\ndescription: Synthetic.\n---\n";
        fs::write(user.join("maestro.agent.md"), valid).unwrap();
        let preview = adapter
            .preview(&fixture.project, &snapshot, false, trust)
            .unwrap();
        fs::write(
            user.join("renamed.agent.md"),
            valid.replace("neighbour", "maestro"),
        )
        .unwrap();
        assert!(preview.apply(&fixture.project, trust).is_err());
        assert!(
            adapter
                .preview(&fixture.project, &snapshot, false, trust)
                .is_err()
        );
        assert!(!fixture.project.join(".github").exists());
        fs::remove_file(user.join("renamed.agent.md")).unwrap();
        fs::create_dir_all(fixture.project.join(".github/agents")).unwrap();
        let project_shadow = fixture.project.join(".github/agents/renamed.agent.md");
        fs::write(&project_shadow, valid.replace("neighbour", "'maestro'")).unwrap();
        assert!(
            adapter
                .preview(&fixture.project, &snapshot, false, trust)
                .is_err()
        );
        fs::remove_file(project_shadow).unwrap();
        adapter
            .preview(&fixture.project, &snapshot, false, trust)
            .unwrap()
            .apply(&fixture.project, trust)
            .unwrap();
        assert_eq!(
            fs::read_to_string(user.join("maestro.agent.md")).unwrap(),
            valid
        );
    });
}
