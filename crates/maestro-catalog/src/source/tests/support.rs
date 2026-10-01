//! An in-memory [`SourceTree`] adapter holding the valid synthetic catalog,
//! and the helpers that mutate and check it.

use crate::source::kinds::legacy as builtin;
use crate::{
    limits::Limits,
    source::{
        Catalog, Entry, EntryKind, Known, Refusal, Registry, Resource, SourceTree,
        builtin as scoped_builtin, check, frozen_rows,
    },
};
use maestro_settings::Registry as SettingsRegistry;
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};

/// The synthetic fixture shared with the command's process tests.
macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/catalog/source/",
            $name
        ))
    };
}

/// The files of the valid catalog: where each lives and its fixture.
pub(super) const VALID: [(&str, &str); 10] = [
    ("agents/base/valid.agent.md", fixture!("valid.agent.md")),
    (
        "agents/base/valid.maestro.toml",
        fixture!("valid.maestro.toml"),
    ),
    (
        "skills/valid-skill/SKILL.md",
        fixture!("valid-skill/SKILL.md"),
    ),
    (
        "instructions/valid.instructions.md",
        fixture!("valid.instructions.md"),
    ),
    (
        "instructions/valid.maestro.toml",
        fixture!("valid.instructions.maestro.toml"),
    ),
    ("mcp/maestro.toml", fixture!("mcp.toml")),
    ("presets/knowledge-client.toml", fixture!("preset.toml")),
    (
        "bootstrap/base/README.md",
        "Copied as data {{ never rendered }}\n",
    ),
    ("README.md", "# Synthetic catalog\n"),
    ("CODEOWNERS", "* @synthetic/knowledge\n"),
];

/// The invalid agent fixture: `metadata:` in its profile, no Boundaries.
pub(super) const INVALID_AGENT: &str = fixture!("invalid.agent.md");

/// A catalog held in memory.
#[derive(Debug, Clone, Default)]
pub(crate) struct MemoryTree {
    /// Each file's relative path and bytes.
    files: BTreeMap<String, Vec<u8>>,
    /// Paths listed as links.
    links: BTreeSet<String>,
    /// Files and directories whose read or listing fails.
    unreadable: BTreeSet<String>,
}

impl MemoryTree {
    /// The valid synthetic catalog.
    pub(crate) fn valid() -> Self {
        VALID
            .into_iter()
            .fold(Self::default(), |tree, (path, text)| tree.with(path, text))
    }

    /// This catalog with `path` holding `text`.
    pub(crate) fn with(mut self, path: &str, text: &str) -> Self {
        self.files.insert(path.to_owned(), text.as_bytes().to_vec());
        self
    }

    /// This catalog with `path` holding `bytes`.
    pub(super) fn with_bytes(mut self, path: &str, bytes: &[u8]) -> Self {
        self.files.insert(path.to_owned(), bytes.to_vec());
        self
    }

    /// This catalog without `path`.
    pub(super) fn without(mut self, path: &str) -> Self {
        assert!(self.files.remove(path).is_some(), "no {path}");
        self
    }

    /// This catalog with a link at `path`.
    pub(super) fn with_link(mut self, path: &str) -> Self {
        self.links.insert(path.to_owned());
        self
    }

    /// This catalog where reading or listing `path` fails.
    pub(super) fn with_unreadable(mut self, path: &str) -> Self {
        self.unreadable.insert(path.to_owned());
        self
    }

    /// This catalog with the one `from` in `path` replaced by `to`.
    pub(super) fn edit(self, path: &str, from: &str, to: &str) -> Self {
        let text = self.text(path);
        assert_eq!(text.matches(from).count(), 1, "{from:?} in {path}");
        self.with(path, &text.replacen(from, to, 1))
    }

    /// The text of `path`.
    pub(super) fn text(&self, path: &str) -> String {
        String::from_utf8(self.files[path].clone()).unwrap()
    }
}

impl SourceTree for MemoryTree {
    fn list(&self, directory: &str) -> io::Result<Vec<Entry>> {
        if self.unreadable.contains(directory) {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let prefix = if directory.is_empty() {
            String::new()
        } else {
            format!("{directory}/")
        };
        let mut entries = BTreeMap::new();
        for path in self.files.keys().chain(&self.links) {
            let Some(rest) = path.strip_prefix(&prefix) else {
                continue;
            };
            let (name, kind) = match rest.split_once('/') {
                Some((name, _)) => (name, EntryKind::Directory),
                None if self.links.contains(path) => (rest, EntryKind::Unsupported),
                None => (rest, EntryKind::File),
            };
            entries.insert(name.to_owned(), kind);
        }
        if entries.is_empty() {
            return Err(io::Error::from(io::ErrorKind::NotFound));
        }
        Ok(entries
            .into_iter()
            .map(|(name, kind)| Entry { name, kind })
            .collect())
    }

    fn read(&self, file: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        if self.unreadable.contains(file) {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let bytes = self
            .files
            .get(file)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let end = usize::try_from(max_bytes.saturating_add(1)).unwrap_or(usize::MAX);
        Ok(bytes.iter().take(end).copied().collect())
    }
}

/// Parses and checks the model-card fixture through the production checker.
pub(crate) fn checked_model_card(text: &str) -> Resource {
    check_by(
        &MemoryTree::default().with("core/llm/models/embedder/synthetic.toml", text),
        &scoped_builtin().unwrap(),
        &Limits::PRODUCTION,
    )
    .unwrap()
    .resources
    .into_iter()
    .find(|resource| resource.id.kind == "model-card")
    .unwrap()
}

/// Checks `tree` against `registry` under `limits`, with the frozen 08 rows
/// and the shipped settings.
pub(super) fn check_by(
    tree: &dyn SourceTree,
    registry: &Registry,
    limits: &Limits,
) -> Result<Catalog, Refusal> {
    let (rows, settings) = (frozen_rows(), SettingsRegistry::built_in().unwrap());
    let known = Known {
        rows: &rows,
        settings: &settings,
    };
    check(tree, registry, limits, known)
}

/// Checks `tree` under `limits` against the built-in kinds, the frozen 08
/// rows and the shipped settings.
pub(crate) fn check_under(tree: &MemoryTree, limits: &Limits) -> Result<Catalog, Refusal> {
    check_by(tree, &builtin().unwrap(), limits)
}

/// Checks that each case's catalog is refused by the built-in kinds with a
/// diagnostic line containing its expected text.
pub(super) fn assert_refused(cases: Vec<(&str, MemoryTree, &str)>) {
    assert_refused_by(&builtin().unwrap(), cases);
}

/// Checks that each case's catalog is refused by `registry` with a
/// diagnostic line containing its expected text, and reports every case
/// that was not.
pub(super) fn assert_refused_by(registry: &Registry, cases: Vec<(&str, MemoryTree, &str)>) {
    let failures: Vec<String> = cases
        .into_iter()
        .filter_map(|(name, tree, expected)| {
            let lines = match check_by(&tree, registry, &Limits::PRODUCTION) {
                Ok(catalog) => vec![format!("accepted: {catalog:?}")],
                Err(refusal) => refusal
                    .diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            };
            (!lines.iter().any(|line| line.contains(expected)))
                .then(|| format!("{name}: expected {expected:?} in {lines:#?}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
