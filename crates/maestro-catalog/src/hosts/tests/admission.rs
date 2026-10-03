//! Host source guards are stimulated without upstream inventory validation.
use super::super::SourceSnapshot;
use crate::bootstrap::{Preset, PresetPort, SourceFile};
use std::collections::BTreeMap;

struct Port(Vec<Preset>);
impl PresetPort for Port {
    fn resolve(&self, _: &[String]) -> Result<Vec<Preset>, String> {
        Ok(self.0.clone())
    }
}
fn preset(sources: &[(&str, &str, &[u8])]) -> Preset {
    Preset {
        name: "synthetic".into(),
        files: BTreeMap::new(),
        source_files: sources
            .iter()
            .map(|(path, id, bytes)| {
                (
                    path.to_string(),
                    SourceFile {
                        id: id.to_string(),
                        revision: None,
                        bytes: bytes.to_vec(),
                    },
                )
            })
            .collect(),
        areas: Vec::new(),
        tools: Vec::new(),
        bindings: Vec::new(),
        source_root: None,
    }
}
const VALID: &[u8] = b"---\nname: maestro\ntools: [view]\n---\nSynthetic body.\n";

#[test]
fn hosts_copilot_admission_and_shadow_guard_neighbours() {
    let canonical = ("core/maestro.agent.md", "agent:core/maestro", VALID);
    let selected = ["synthetic".to_owned()];
    let port = Port(vec![preset(&[canonical])]);
    assert!(SourceSnapshot::resolve(&port, &selected).is_ok());
    assert!(SourceSnapshot::resolve(&port, &[]).is_err());
    for sources in [
        vec![],
        vec![canonical, ("other.agent.md", "agent:core/maestro", VALID)],
    ] {
        assert!(SourceSnapshot::resolve(&Port(vec![preset(&sources)]), &selected).is_err());
    }
    for text in [
        b"no frontmatter".as_slice(),
        b"---\nname: maestro\n---\n\xff",
        b"---\nname: maestro\nmissing delimiter",
        b"---\ntools: view\n---\nbody",
        b"---\ntools: [1]\n---\nbody",
    ] {
        let port = Port(vec![preset(&[(canonical.0, canonical.1, text)])]);
        assert!(SourceSnapshot::resolve(&port, &selected).is_err());
    }
    let duplicate = Port(vec![preset(&[canonical]), preset(&[canonical])]);
    assert!(SourceSnapshot::resolve(&duplicate, &selected).is_ok());
    let different = preset(&[(canonical.0, canonical.1, b"---\nname: maestro\n---\nother")]);
    assert!(
        SourceSnapshot::resolve(&Port(vec![preset(&[canonical]), different]), &selected).is_err()
    );
    let irrelevant = preset(&[
        ("note.txt", "agent:core/maestro", b"not an agent"),
        canonical,
    ]);
    assert!(SourceSnapshot::resolve(&Port(vec![irrelevant]), &selected).is_ok());
}

#[test]
fn hosts_copilot_adapter_dispatch_and_source_check_guards() {
    use super::super::{
        Copilot,
        copilot::{NativeProjection as _, adapter},
    };
    use crate::files::tests::support::with_trust;
    use maestro_test_scratch::scratch_directory;
    use std::fs;
    let adapter = adapter("copilot").unwrap();
    assert!(
        adapter
            .initialization_instructions("session")
            .contains("Maestro MCP knowledge tools")
    );
    let root = scratch_directory().unwrap().canonicalize().unwrap();
    let selected = ["synthetic".to_owned()];
    let canonical = ("core/maestro.agent.md", "agent:core/maestro", VALID);
    with_trust(&root, |trust| {
        let mut bad = preset(&[("", "sidecar", b"inert"), canonical]);
        bad.source_root = Some(root.clone());
        let snapshot = SourceSnapshot::resolve(&Port(vec![bad]), &selected).unwrap();
        let preview = Copilot::new(None)
            .preview(&root, &snapshot, false, trust)
            .unwrap();
        assert!(preview.apply(&root, trust).is_err());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        let no_tools = (
            canonical.0,
            canonical.1,
            b"---\nname: maestro\n---\nbody".as_slice(),
        );
        let snapshot =
            SourceSnapshot::resolve(&Port(vec![preset(&[no_tools])]), &selected).unwrap();
        Copilot::new(None)
            .preview(&root, &snapshot, false, trust)
            .unwrap()
            .apply(&root, trust)
            .unwrap();
        assert!(root.join(".github/agents/maestro.agent.md").exists());
    });
    fs::remove_dir_all(root).unwrap();
}
