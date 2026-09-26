//! The import streams: it holds one manifest line and one document at a
//! time, which an in-memory corpus checks at each read, and the check
//! catches an import that reads the manifest ahead of its documents.

use crate::{
    RelativePath,
    collection::Declaration,
    import::{collection::import_corpora, corpus::Corpus},
};
use maestro_kernel::{artifact::Digest, scope::Right, store::Database};
use serde_json::json;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    env, fs,
    io::{self, BufRead, Read},
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// How many documents the streaming test imports.
const DOCUMENTS: usize = 40;

/// A new empty directory under the platform's temporary directory for the
/// kernel's data, removed with everything in it when dropped, after the
/// database a test opened in it.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-knowledge-streaming-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// What an in-memory corpus saw of the reads made of it.
#[derive(Debug, Default)]
struct Seen {
    /// The line the manifest handed out last, by its index.
    last: Option<usize>,
    /// How many documents were read.
    documents: usize,
    /// Each read that broke the rule of one line and one document at a
    /// time.
    violations: Vec<String>,
}

/// A corpus in memory whose manifest hands out one line at a time, and
/// which records each document read that is not the one the line handed out
/// last declares: an import that read ahead, or held two lines at once.
#[derive(Debug)]
struct Memory {
    /// Each line with its newline, after the path of the document it
    /// declares.
    lines: Vec<(String, Vec<u8>)>,
    /// Each document's bytes, by path.
    documents: BTreeMap<String, Vec<u8>>,
    /// What the reads made of it did.
    seen: RefCell<Seen>,
}

impl Memory {
    /// The corpus of `count` small documents, each declared truly.
    fn of(count: usize) -> Self {
        let mut lines = Vec::new();
        let mut documents = BTreeMap::new();
        for index in 0..count {
            let path = format!("pages/{index}.md");
            let bytes = format!("# Page {index}\n\nWhat page {index} says.\n").into_bytes();
            let line = json!({
                "schema": "maestro-corpus/1",
                "path": path,
                "sha256": Digest::of(&bytes).as_str(),
                "bytes": bytes.len(),
                "source_ref": format!("https://example.org/pages/{index}"),
                "title": format!("Page {index}"),
                "source_kind": "guide",
            });
            lines.push((path.clone(), format!("{line}\n").into_bytes()));
            documents.insert(path, bytes);
        }
        Self {
            lines,
            documents,
            seen: RefCell::default(),
        }
    }
}

impl Corpus for Memory {
    fn manifest(&self) -> io::Result<impl BufRead> {
        Ok(LineByLine {
            corpus: self,
            index: 0,
            offset: 0,
        })
    }

    fn document(&self, path: &RelativePath) -> io::Result<Vec<u8>> {
        let mut seen = self.seen.borrow_mut();
        seen.documents += 1;
        let last = seen
            .last
            .and_then(|index| self.lines.get(index))
            .map(|(declared, _)| declared.as_str());
        if last != Some(path.as_str()) {
            let violation = format!(
                "{} read while the last line handed out declares {last:?}",
                path.as_str()
            );
            seen.violations.push(violation);
        }
        self.documents
            .get(path.as_str())
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
}

/// A manifest in memory that fills its buffer with one line at a time, and
/// tells its corpus which line it handed out last.
struct LineByLine<'corpus> {
    /// The corpus it belongs to.
    corpus: &'corpus Memory,
    /// The line it is handing out.
    index: usize,
    /// How much of that line was consumed.
    offset: usize,
}

impl Read for LineByLine<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let available = self.fill_buf()?;
        let count = available.len().min(buffer.len());
        buffer[..count].copy_from_slice(&available[..count]);
        self.consume(count);
        Ok(count)
    }
}

impl BufRead for LineByLine<'_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let lines = &self.corpus.lines;
        while lines
            .get(self.index)
            .is_some_and(|(_, line)| self.offset == line.len())
        {
            self.index += 1;
            self.offset = 0;
        }
        match lines.get(self.index) {
            Some((_, line)) => {
                self.corpus.seen.borrow_mut().last = Some(self.index);
                Ok(&line[self.offset..])
            }
            None => Ok(&[]),
        }
    }

    fn consume(&mut self, amount: usize) {
        self.offset += amount;
    }
}

/// The declaration of the collection `pages`, with one source, `web`.
fn declaration() -> Declaration {
    json!({
        "schema": "maestro-collection/1",
        "id": "pages",
        "title": "Generated pages",
        "visibility": "public",
        "profiles": {
            "extraction": "technical-html/1",
            "chunking": "structural-500-700/1",
            "embedding": "embed:winner",
            "sparse": "bm25-en-fr/1",
        },
        "quality": { "ledger": "quality/ledger.jsonl" },
        "sources": [{
            "id": "web",
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

#[test]
fn the_import_holds_one_manifest_line_and_one_document_at_a_time() {
    let scratch = Scratch::new();
    let database = Database::open_in(&scratch.0).unwrap();
    let workspace = "workspace/default".parse().unwrap();
    database
        .grant("tester", &workspace, Right::Read, "test")
        .unwrap();
    let scopes = database.visible("tester").unwrap();
    let declaration = declaration();
    let corpora = [(&declaration.sources[0], Memory::of(DOCUMENTS))];
    let report = import_corpora(&database, &scopes, &declaration, &corpora).unwrap();
    assert_eq!(report.imported, u64::try_from(DOCUMENTS).unwrap());
    let seen = corpora[0].1.seen.borrow();
    assert_eq!(seen.documents, DOCUMENTS);
    assert!(seen.violations.is_empty(), "{:#?}", seen.violations);
}

#[test]
fn a_manifest_read_ahead_of_its_documents_is_caught() {
    let corpus = Memory::of(2);
    let mut manifest = corpus.manifest().unwrap();
    let mut lines = Vec::new();
    manifest.read_until(b'\n', &mut lines).unwrap();
    manifest.read_until(b'\n', &mut lines).unwrap();
    let first: RelativePath = serde_json::from_value(json!("pages/0.md")).unwrap();
    let second: RelativePath = serde_json::from_value(json!("pages/1.md")).unwrap();
    assert!(corpus.document(&first).is_ok());
    assert!(corpus.document(&second).is_ok());
    let seen = corpus.seen.borrow();
    assert_eq!(seen.documents, 2);
    let [violation] = seen.violations.as_slice() else {
        panic!("{:#?}", seen.violations);
    };
    assert!(
        violation.starts_with("pages/0.md read while"),
        "{violation}"
    );
}
