//! Determinism, inertness, exact closures and limit boundaries.

use crate::{
    bundle::{Bundle, compile},
    limits::Limits,
    source::{
        Known, Refusal, Registry, SourceTree, builtin, frozen_rows, tests::support::MemoryTree,
    },
};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::files::digest;
use crate::source::{Entry, tests::registry::glossary};
use std::{cell::RefCell, collections::BTreeSet, io};

/// Synthetic release commit, unrelated to any real repository.
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

/// Run the production compiler with injected budgets.
pub(super) fn run(
    tree: &dyn SourceTree,
    registry: &Registry,
    limits: &Limits,
) -> Result<Bundle, Refusal> {
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().unwrap();
    compile(
        tree,
        registry,
        limits,
        Known {
            rows: &rows,
            settings: &settings,
            today: 0,
        },
        COMMIT,
    )
}

/// Read ustar headers directly: tests must not import the archive library.
pub(super) fn entries(bytes: &[u8]) -> BTreeMap<String, &[u8]> {
    let mut result = BTreeMap::new();
    let mut at = 0;
    let text = |bytes: &[u8]| {
        String::from_utf8(
            bytes
                .iter()
                .copied()
                .take_while(|byte| *byte != 0)
                .collect(),
        )
        .unwrap()
    };
    while bytes[at] != 0 {
        let header = &bytes[at..at + 512];
        let name = text(&header[..100]);
        let prefix = text(&header[345..500]);
        let path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        assert!(
            result
                .keys()
                .next_back()
                .is_none_or(|previous| previous < &path)
        );
        let size = usize::from_str_radix(text(&header[124..136]).trim(), 8).unwrap();
        assert_eq!(text(&header[100..108]).trim(), "0000644");
        assert_eq!(text(&header[108..116]).trim(), "0000000");
        assert_eq!(text(&header[116..124]).trim(), "0000000");
        assert_eq!(text(&header[136..148]).trim(), "00000000000");
        assert_eq!(header[156], b'0');
        assert!(header[265..329].iter().all(|byte| *byte == 0));
        assert!(
            result
                .insert(path, &bytes[at + 512..at + 512 + size])
                .is_none()
        );
        at += 512 + size.div_ceil(512) * 512;
    }
    assert_eq!(bytes.len() - at, 1024);
    assert!(bytes[at..].iter().all(|byte| *byte == 0));
    result
}

#[test]
fn deterministic_bytes_and_changed_definition_digest() {
    let tree = MemoryTree::valid();
    let registry = builtin().unwrap();
    let first = run(&tree, &registry, &Limits::PRODUCTION).unwrap();
    let second = run(&tree, &registry, &Limits::PRODUCTION).unwrap();
    assert_eq!(first.bytes, second.bytes);
    assert_eq!(first.digest, second.digest);
    assert_eq!(first.digest, digest(&first.bytes));
    let files = entries(&first.bytes);
    let document: Value = serde_json::from_slice(files["bundle.json"]).unwrap();
    assert_eq!(document["schema"], "maestro-bundle/1");
    assert_eq!(document["source_commit"], COMMIT);
    assert_eq!(
        document["closures"]["preset:knowledge-client"],
        serde_json::json!([
            "agent:core/valid",
            "instructions:core/valid",
            "package:common",
            "package:core",
            "preset:knowledge-client",
            "skill:common/valid-skill"
        ])
    );
    assert_eq!(
        document["resources"]["agent:core/valid"]["owners"],
        serde_json::json!(["@synthetic/knowledge"])
    );
    assert_eq!(
        document["resources"]["agent:core/valid"]["maturity"],
        "reviewed"
    );
    assert_eq!(document["requires"]["runtime"], serde_json::json!({}));
    assert_eq!(document["requires"]["features"], serde_json::json!([]));
    assert_eq!(
        document["requires"]["tool_contracts"],
        serde_json::json!([])
    );
    assert_eq!(
        document["policy_digest"],
        "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
    );
    assert_eq!(serde_json::to_vec(&document).unwrap(), files["bundle.json"]);
    let original = String::from_utf8(tree.read("package.toml", 4096).unwrap()).unwrap();
    let changed = tree.with(
        "package.toml",
        &original.replace("Synthetic area", "Changed area"),
    );
    assert_ne!(
        first.digest,
        run(&changed, &registry, &Limits::PRODUCTION)
            .unwrap()
            .digest
    );
}

#[test]
fn archive_limits_accept_exact_and_refuse_one_past() {
    let tree = MemoryTree::valid();
    let registry = builtin().unwrap();
    let bundle = run(&tree, &registry, &Limits::PRODUCTION).unwrap();
    let files = entries(&bundle.bytes);
    let max_entry = files
        .values()
        .map(|bytes| bytes.len() as u64)
        .max()
        .unwrap();
    let exact = Limits {
        archive_entry_bytes: max_entry,
        archive_total_bytes: bundle.bytes.len() as u64,
        ..Limits::PRODUCTION
    };
    assert!(run(&tree, &registry, &exact).is_ok());
    for (limits, message) in [
        (
            Limits {
                archive_entry_bytes: max_entry - 1,
                ..exact
            },
            "entry bytes",
        ),
        (
            Limits {
                archive_total_bytes: bundle.bytes.len() as u64 - 1,
                ..exact
            },
            "stream bytes",
        ),
        (
            Limits {
                archive_total_bytes: files.values().map(|bytes| bytes.len() as u64).sum::<u64>()
                    - 1,
                ..exact
            },
            "aggregate bytes",
        ),
    ] {
        assert!(
            run(&tree, &registry, &limits)
                .unwrap_err()
                .to_string()
                .contains(message)
        );
    }
}

/// A minimal source-valid catalog makes the archive count boundary observable:
/// one source file, plus the manifest, without counting source directories.
pub(super) fn minimal() -> MemoryTree {
    let tree = MemoryTree::valid();
    MemoryTree::default().with(
        "package.toml",
        &String::from_utf8(tree.read("package.toml", 4096).unwrap()).unwrap(),
    )
}

/// Container depth, with scalar values contributing no container level.
fn depth(value: &Value) -> usize {
    match value {
        Value::Object(fields) => 1 + fields.values().map(depth).max().unwrap_or(0),
        Value::Array(items) => 1 + items.iter().map(depth).max().unwrap_or(0),
        _ => 0,
    }
}

#[test]
fn entry_count_and_manifest_depth_boundaries() {
    let tree = minimal();
    let registry = builtin().unwrap();
    let limits = Limits {
        archive_entries: 2,
        ..Limits::PRODUCTION
    };
    let bundle = run(&tree, &registry, &limits).unwrap();
    assert_eq!(entries(&bundle.bytes).len(), 2);
    let below = Limits {
        archive_entries: 1,
        ..limits
    };
    assert!(
        run(&tree, &registry, &below)
            .unwrap_err()
            .to_string()
            .contains("archive entry count")
    );
    let nesting =
        depth(&serde_json::from_slice::<Value>(entries(&bundle.bytes)["bundle.json"]).unwrap());
    assert!(
        run(
            &tree,
            &registry,
            &Limits {
                manifest_depth: nesting,
                ..limits
            }
        )
        .is_ok()
    );
    assert!(
        run(
            &tree,
            &registry,
            &Limits {
                manifest_depth: nesting - 1,
                ..limits
            }
        )
        .unwrap_err()
        .to_string()
        .contains("manifest nesting")
    );
}

#[test]
fn descriptor_only_extension_preserves_data_and_references() {
    let tree = MemoryTree::valid().with(
        "glossaries/evidence.toml",
        "term = 'evidence'\nweight = 0.6\n[source]\ntitle = 'Synthetic glossary'\n\
         page = 3\npublic = true\n[metadata]\nschema = 'maestro-source/2'\n\
         maturity = 'reviewed'\nrows = ['chat.M036 objects']\nworkflows = ['ctm-question']\n",
    );
    let preset =
        String::from_utf8(tree.read("presets/knowledge-client.toml", 4096).unwrap()).unwrap();
    let tree = tree.with(
        "presets/knowledge-client.toml",
        &preset.replace(
            "\"agent:core/valid\"]",
            "\"agent:core/valid\", \"glossary:common/evidence\"]",
        ),
    );
    let mut registry = builtin().unwrap();
    registry.register(glossary()).unwrap();
    let bundle = run(&tree, &registry, &Limits::PRODUCTION).unwrap();
    let document: Value = serde_json::from_slice(entries(&bundle.bytes)["bundle.json"]).unwrap();
    assert_eq!(
        document["resources"]["glossary:common/evidence"]["fields"]["weight"],
        0.6
    );
    assert_eq!(
        document["resources"]["glossary:common/evidence"]["fields"]["source"],
        serde_json::json!({"title": "Synthetic glossary", "page": 3, "public": true})
    );
    assert!(
        document["closures"]["preset:knowledge-client"]
            .as_array()
            .unwrap()
            .contains(&Value::from("glossary:common/evidence"))
    );
    assert!(run(&tree, &builtin().unwrap(), &Limits::PRODUCTION).is_err());
}

#[test]
fn bundle_preserves_config_closure_and_runtime() {
    let tree = MemoryTree::valid();
    let core = String::from_utf8(tree.read("core/package.toml", 4096).unwrap()).unwrap();
    let runtime = format!("={}", env!("CARGO_PKG_VERSION"));
    let tree = tree
        .with(
            "core/package.toml",
            &format!("runtime = {runtime:?}\n{core}"),
        )
        .with(
            "settings/defaults.toml",
            "schema = 'maestro-preferences/1'\nlanguage = 'en'\n",
        );
    let bundle = run(&tree, &builtin().unwrap(), &Limits::PRODUCTION).unwrap();
    let files = entries(&bundle.bytes);
    assert_eq!(
        files["settings/defaults.toml"],
        tree.read("settings/defaults.toml", 4096).unwrap()
    );
    let document: Value = serde_json::from_slice(files["bundle.json"]).unwrap();
    assert_eq!(
        document["entries"]["settings/defaults.toml"]["digest"],
        digest(files["settings/defaults.toml"])
    );
    assert_eq!(
        document["requires"]["runtime"],
        serde_json::json!({"package:core": runtime})
    );
    assert_eq!(document["id"], "common");
    assert_eq!(document["version"], "1.2.3");
}

#[test]
fn implicit_standard_edges_are_preserved() {
    let tree = MemoryTree::valid().with(
        "standards/security/package.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/catalog/bootstrap/owner-local/",
            "standards/security/package.toml"
        )),
    );
    let bundle = run(&tree, &builtin().unwrap(), &Limits::PRODUCTION).unwrap();
    assert!(
        bundle.manifest.closures["preset:knowledge-client"]
            .contains(&"standard:security".to_owned())
    );
    assert!(
        bundle.manifest.resources["preset:knowledge-client"]
            .requires
            .contains(&"standard:security".to_owned())
    );
}

/// A source adapter that changes listing order and refuses any second read.
struct OnceTree {
    /// Original synthetic input.
    tree: MemoryTree,
    /// Already-read relative paths.
    reads: RefCell<BTreeSet<String>>,
}

impl SourceTree for OnceTree {
    fn list(&self, directory: &str) -> io::Result<Vec<Entry>> {
        let mut entries = self.tree.list(directory)?;
        entries.reverse();
        Ok(entries)
    }
    fn read(&self, path: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        assert!(
            self.reads.borrow_mut().insert(path.to_owned()),
            "source reread: {path}"
        );
        self.tree.read(path, max_bytes)
    }
}

#[test]
fn reordered_inputs_share_the_original_checked_snapshot() {
    let tree = MemoryTree::valid();
    let registry = builtin().unwrap();
    let expected = run(&tree, &registry, &Limits::PRODUCTION).unwrap();
    let reordered = OnceTree {
        tree,
        reads: RefCell::new(BTreeSet::new()),
    };
    assert_eq!(
        expected.bytes,
        run(&reordered, &registry, &Limits::PRODUCTION)
            .unwrap()
            .bytes
    );
}

#[test]
fn missing_catalog_root_package_refuses() {
    assert!(
        run(
            &MemoryTree::default().with(
                "core/package.toml",
                &String::from_utf8(MemoryTree::valid().read("core/package.toml", 4096).unwrap())
                    .unwrap()
            ),
            &builtin().unwrap(),
            &Limits::PRODUCTION
        )
        .unwrap_err()
        .to_string()
        .contains("catalog root package")
    );
}

#[test]
fn provenance_refuses_invalid_commit() {
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().unwrap();
    let known = Known {
        rows: &rows,
        settings: &settings,
        today: 0,
    };
    for bad in [
        "",
        "abc",
        "G123456789abcdef0123456789abcdef01234567",
        "A123456789abcdef0123456789abcdef01234567",
    ] {
        assert!(
            compile(
                &MemoryTree::valid(),
                &builtin().unwrap(),
                &Limits::PRODUCTION,
                known,
                bad
            )
            .unwrap_err()
            .to_string()
            .contains("source_commit")
        );
    }
    let long = "a".repeat(64);
    assert!(
        compile(
            &MemoryTree::valid(),
            &builtin().unwrap(),
            &Limits::PRODUCTION,
            known,
            &long
        )
        .is_ok()
    );
}
