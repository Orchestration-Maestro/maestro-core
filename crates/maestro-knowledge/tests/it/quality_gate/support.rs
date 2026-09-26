//! What the gate's tests share: a scratch directory holding a corpus and a
//! kernel, the corpus imported (T019) as the collection `garden` by a
//! principal that reads the whole default workspace, and what the kernel then
//! records of each revision.

use maestro_kernel::{
    artifact::Digest,
    binding::Bindings,
    document::{Disposition, Revision},
    journal::{Event, Filter},
    scope::{Right, ScopeSet},
    store::Database,
};
use maestro_knowledge::{collection::Declaration, import};
use serde_json::{Value, json};
use std::{
    env,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Who the gate names as the decider of what its checks decide.
pub(super) const GATE: &str = "quality-gate/1";

/// The type of the event of a revision held back.
pub(super) const HELD: &str = "maestro.knowledge.revision.held.v1";

/// A page whose body is a single link: near-empty.
pub(super) const EMPTY: &str = "# Garden party\n\n[See every event](https://example.org/events)\n";

/// A page no check flags.
pub(super) const CLEAN: &str = "# Backups\n\nThe backup runs every night at two in the morning.\n";

/// A page whose front matter contradicts its title: canonicalization fails.
pub(super) const FAILED: &str = "---\ntitle: Another page\n---\n# Page\n\n\
    What the page says about the backups of the database.\n";

/// A page of about 300 characters with one replacement character: a warning.
pub(super) const REPLACED: &str = "# Owners\n\nThe \u{fffd}owner of the files is the account \
    that runs the agent. It must stay the same after an upgrade of the host, or the agent \
    cannot read the files it wrote before, and every job that reads them ends in error until \
    someone gives the files back to the account that runs the agent now.\n";

/// A new directory under the platform's temporary directory, removed with
/// everything in it when dropped, after the database a test opened in it:
/// `corpus/` holds the pages and their manifest, and `kernel/` the kernel.
pub(super) struct Scratch(PathBuf);

impl Scratch {
    /// A directory of its own for one test.
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-knowledge-quality-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("corpus")).unwrap();
        Self(path)
    }

    /// The directory.
    pub(super) fn path(&self) -> &Path {
        &self.0
    }

    /// The kernel's database, in `kernel/`.
    pub(super) fn database(&self) -> Database {
        Database::open_in(&self.0.join("kernel")).unwrap()
    }

    /// Writes each page of `pages`, a name and its Markdown, as `<name>.md`
    /// from `https://example.org/<name>`, and the manifest that lists them.
    pub(super) fn pages(&self, pages: &[(&str, &str)]) {
        let lines: Vec<(String, String, &str)> = pages
            .iter()
            .map(|(name, markdown)| {
                (
                    format!("{name}.md"),
                    format!("https://example.org/{name}"),
                    *markdown,
                )
            })
            .collect();
        self.lines(&lines);
    }

    /// Writes each file of `lines`, a path, the `source_ref` its line gives
    /// it and its Markdown, and the manifest that lists them in that order,
    /// each a guide of the set `notes`, release 1.0.
    pub(super) fn lines(&self, lines: &[(String, String, &str)]) {
        let mut manifest = String::new();
        for (path, source_ref, markdown) in lines {
            fs::write(self.0.join("corpus").join(path), markdown).unwrap();
            let line = json!({
                "schema": "maestro-corpus/1",
                "path": path,
                "sha256": Digest::of(markdown.as_bytes()).as_str(),
                "bytes": markdown.len(),
                "source_ref": source_ref,
                "title": format!("Notes on {path}"),
                "source_kind": "guide",
                "set": "notes",
                "version": "1.0",
            });
            writeln!(manifest, "{line}").unwrap();
        }
        fs::write(self.0.join("corpus").join("manifest.jsonl"), manifest).unwrap();
    }

    /// Imports the collection `garden`, whose one source's manifest this
    /// directory holds, and returns what the importing principal reads: the
    /// whole default workspace.
    pub(super) fn import(&self, database: &Database) -> ScopeSet {
        let scopes = everything(database);
        let bindings: Bindings = format!("corpus_root = '{}'\n", self.0.join("corpus").display())
            .parse()
            .unwrap();
        import::import(database, &scopes, &declaration(), &bindings).unwrap();
        scopes
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// The declaration of `garden`, whose one source, `docs`, imports the
/// manifest `manifest.jsonl` of the binding `corpus_root`.
fn declaration() -> Declaration {
    json!({
        "schema": "maestro-collection/1",
        "id": "garden",
        "title": "The garden",
        "visibility": "public",
        "profiles": {
            "extraction": "technical-html/1",
            "chunking": "structural-500-700/1",
            "embedding": "embed:winner",
            "sparse": "bm25-en-fr/1",
        },
        "quality": { "ledger": "quality/ledger.jsonl" },
        "sources": [{
            "id": "docs",
            "kind": "import",
            "sync": "manual",
            "manifest": { "binding": "corpus_root", "path": "manifest.jsonl" },
        }],
        "evals": { "suite": "evals" },
    })
    .to_string()
    .parse()
    .unwrap()
}

/// What the principal `tester` reads once granted the default workspace.
pub(super) fn everything(database: &Database) -> ScopeSet {
    let workspace = "workspace/default".parse().unwrap();
    database
        .grant("tester", &workspace, Right::Read, "test")
        .unwrap();
    database.visible("tester").unwrap()
}

/// Every revision of `garden`, failed ones included, in record order.
pub(super) fn revisions(database: &Database, scopes: &ScopeSet) -> Vec<Revision> {
    database.revisions(scopes, "garden").unwrap()
}

/// The revision of `garden` whose original Markdown is `markdown`.
pub(super) fn revision_of(database: &Database, scopes: &ScopeSet, markdown: &str) -> Revision {
    let original = Digest::of(markdown.as_bytes());
    revisions(database, scopes)
        .into_iter()
        .find(|revision| revision.original_digest == original)
        .unwrap()
}

/// The disposition the kernel records of the revision of `markdown`.
pub(super) fn disposition_of(
    database: &Database,
    scopes: &ScopeSet,
    markdown: &str,
) -> Disposition {
    let revision = revision_of(database, scopes, markdown);
    database.disposition(scopes, &revision.id).unwrap().unwrap()
}

/// Every event of the stream of `garden`.
pub(super) fn events(database: &Database, scopes: &ScopeSet) -> Vec<Event> {
    let filter = Filter {
        stream: "collection/garden",
        after: 0,
        r#type: None,
    };
    database.events(scopes, &filter).unwrap()
}

/// The data of every event of `events` whose type is `name`, in order.
pub(super) fn data_of(events: &[Event], name: &str) -> Vec<Value> {
    events
        .iter()
        .filter(|event| event.r#type == name)
        .map(|event| event.data.clone())
        .collect()
}
