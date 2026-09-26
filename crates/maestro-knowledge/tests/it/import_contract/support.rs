//! What the import's tests share: a scratch directory holding a corpus and a
//! kernel database, the declaration and bindings that name that corpus, a
//! principal that reads the whole default workspace, and the database and
//! artifact tree read whole, to show what an import wrote.

use maestro_kernel::{
    artifact::Digest,
    binding::Bindings,
    journal::{Event, Filter},
    scope::{Right, ScopeSet},
    store::Database,
};
use maestro_knowledge::{
    collection::Declaration,
    import::{self, Report},
};
use rusqlite::{Connection, OpenFlags, types::Value as Column};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
    time::SystemTime,
};

/// The principal the tests import as.
const TESTER: &str = "tester";

/// A new empty directory under the platform's temporary directory, removed
/// with everything in it when dropped, after the database a test opened in
/// it: `corpus/` holds the documents and their manifests, and `kernel/` the
/// kernel's data.
pub(super) struct Scratch(PathBuf);

impl Scratch {
    /// A directory of its own for one test.
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-knowledge-import-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("corpus")).unwrap();
        Self(path)
    }

    /// The corpus directory, which the binding `corpus_root` names.
    pub(super) fn corpus(&self) -> PathBuf {
        self.0.join("corpus")
    }

    /// Writes `bytes` to the file `path`, relative to the corpus directory,
    /// with the directories it lacks.
    pub(super) fn put(&self, path: &str, bytes: &[u8]) {
        let file = path
            .split('/')
            .fold(self.corpus(), |parent, segment| parent.join(segment));
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, bytes).unwrap();
    }

    /// Writes the manifest `path`, relative to the corpus directory, one
    /// line of `lines` each.
    pub(super) fn manifest_at(&self, path: &str, lines: &[Value]) {
        let text: String = lines.iter().map(|line| line.to_string() + "\n").collect();
        self.put(path, text.as_bytes());
    }

    /// Writes the manifest `manifest.jsonl`, one line of `lines` each.
    pub(super) fn manifest(&self, lines: &[Value]) {
        self.manifest_at("manifest.jsonl", lines);
    }

    /// The kernel's database, in `kernel/`.
    pub(super) fn database(&self) -> Database {
        Database::open_in(&self.0.join("kernel")).unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// A line declaring the document `path` truly, its digest and its size,
/// from `source_ref`, as a guide of release 1.0 titled after its path.
pub(super) fn line(path: &str, bytes: &[u8], source_ref: &str) -> Value {
    json!({
        "schema": "maestro-corpus/1",
        "path": path,
        "sha256": Digest::of(bytes).as_str(),
        "bytes": bytes.len(),
        "source_ref": source_ref,
        "title": format!("Notes on {path}"),
        "source_kind": "guide",
        "version": "1.0",
    })
}

/// The web page `name` of the example site, a `source_ref`.
pub(super) fn page(name: &str) -> String {
    format!("https://example.org/{name}")
}

/// The Markdown of a short document about `topic`.
pub(super) fn markdown(topic: &str) -> Vec<u8> {
    format!("# {topic}\n\nWhat to know about {topic}.\n").into_bytes()
}

/// The declaration of the collection `id`, whose sources `sources` give by
/// id and manifest, each manifest under the binding `corpus_root`.
pub(super) fn declaration_of(id: &str, sources: &[(&str, &str)]) -> Declaration {
    let sources: Vec<Value> = sources
        .iter()
        .map(|(source, manifest)| {
            json!({
                "id": source,
                "kind": "import",
                "sync": "manual",
                "manifest": { "binding": "corpus_root", "path": manifest },
            })
        })
        .collect();
    json!({
        "schema": "maestro-collection/1",
        "id": id,
        "title": format!("The {id} collection"),
        "visibility": "public",
        "profiles": {
            "extraction": "technical-html/1",
            "chunking": "structural-500-700/1",
            "embedding": "embed:winner",
            "sparse": "bm25-en-fr/1",
        },
        "quality": { "ledger": "quality/ledger.jsonl" },
        "sources": sources,
        "evals": { "suite": "evals" },
    })
    .to_string()
    .parse()
    .unwrap()
}

/// The declaration of the collection `id` with one source, `docs`, whose
/// manifest is `manifest.jsonl`.
pub(super) fn declaration(id: &str) -> Declaration {
    declaration_of(id, &[("docs", "manifest.jsonl")])
}

/// The bindings of `scratch`: `corpus_root`, to its corpus directory.
pub(super) fn bindings(scratch: &Scratch) -> Bindings {
    format!("corpus_root = '{}'\n", scratch.corpus().display())
        .parse()
        .unwrap()
}

/// What the tests' principal sees, once granted the whole default workspace.
pub(super) fn everything(database: &Database) -> ScopeSet {
    let workspace = "workspace/default".parse().unwrap();
    database
        .grant(TESTER, &workspace, Right::Read, "test")
        .unwrap();
    database.visible(TESTER).unwrap()
}

/// Imports `declaration`, whose corpus `scratch` holds, as a principal that
/// sees everything.
pub(super) fn import_declared(
    scratch: &Scratch,
    database: &Database,
    declaration: &Declaration,
) -> Report {
    import::import(
        database,
        &everything(database),
        declaration,
        &bindings(scratch),
    )
    .unwrap()
}

/// Imports the collection `id`, whose one source's manifest `scratch` holds.
pub(super) fn import(scratch: &Scratch, database: &Database, id: &str) -> Report {
    import_declared(scratch, database, &declaration(id))
}

/// Every event of the stream of the collection `id`.
pub(super) fn events(database: &Database, id: &str) -> Vec<Event> {
    let stream = format!("collection/{id}");
    let filter = Filter {
        stream: &stream,
        after: 0,
        r#type: None,
    };
    database.events(&everything(database), &filter).unwrap()
}

/// The data of every event of `events` whose type is `name`, in order.
pub(super) fn data_of(events: &[Event], name: &str) -> Vec<Value> {
    events
        .iter()
        .filter(|event| event.r#type == name)
        .map(|event| event.data.clone())
        .collect()
}

/// What the kernel holds: each row of each table but the journal's, by
/// table, and each artifact file, with its size and when it last changed.
#[derive(Debug, PartialEq)]
pub(super) struct Contents {
    /// The rows of each table, in the order of their first column.
    tables: BTreeMap<String, Vec<Vec<Column>>>,
    /// Each artifact file below the store's root, by its path there.
    files: BTreeMap<PathBuf, (u64, SystemTime)>,
}

impl Contents {
    /// What the kernel of `scratch` holds now.
    pub(super) fn of(scratch: &Scratch) -> Self {
        let kernel = scratch.0.join("kernel");
        let connection = Connection::open_with_flags(
            kernel.join("kernel.sqlite3"),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let mut names = connection
            .prepare(
                "SELECT name FROM sqlite_schema
                 WHERE type = 'table' AND name NOT IN ('events', 'cursors') ORDER BY name",
            )
            .unwrap();
        let tables = names
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .map(|name| {
                let name = name.unwrap();
                let rows = rows(&connection, &name);
                (name, rows)
            })
            .collect();
        let mut files = BTreeMap::new();
        let root = kernel.join("artifacts");
        walk(&root, &root, &mut files);
        Self { tables, files }
    }
}

/// Every row of `table` in `connection`, in the order of its first column.
fn rows(connection: &Connection, table: &str) -> Vec<Vec<Column>> {
    let mut statement = connection
        .prepare(&format!("SELECT * FROM {table} ORDER BY 1"))
        .unwrap();
    let width = statement.column_count();
    statement
        .query_map([], |row| (0..width).map(|index| row.get(index)).collect())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// Adds each file below `directory` to `files`, by its path below `root`.
fn walk(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, (u64, SystemTime)>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let metadata = fs::metadata(&path).unwrap();
        if metadata.is_dir() {
            walk(root, &path, files);
        } else {
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            files.insert(relative, (metadata.len(), metadata.modified().unwrap()));
        }
    }
}
