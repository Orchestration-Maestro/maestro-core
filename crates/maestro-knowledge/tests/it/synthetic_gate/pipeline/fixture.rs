//! Loads the pinned synthetic collection and verifies every fixture digest.

use super::super::failure::Failure;
use super::contract::{
    COLLECTION, CorpusDigest, DECLARATION_DIGEST, MANIFEST_DIGEST, SUITE_DIGEST,
};
use maestro_kernel::{artifact::Digest, binding::Bindings};
use maestro_knowledge::{collection::Declaration, corpus::Entry, suite::Suite};
use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process, str,
    sync::atomic::{AtomicU64, Ordering},
};

pub(super) struct FixtureInputs {
    pub(super) root: PathBuf,
    pub(super) declaration: Declaration,
    pub(super) bindings: Bindings,
    pub(super) declaration_digest: String,
    pub(super) manifest_digest: String,
    pub(super) corpus_digests: Vec<CorpusDigest>,
    pub(super) suite: Suite,
}

pub(super) fn load() -> Result<FixtureInputs, Failure> {
    let root = fixture_root()?;
    let declaration_bytes = fs::read(root.join("collection.json"))
        .map_err(|error| Failure::from_error("fixture-declaration", error))?;
    if Digest::of(&declaration_bytes).as_str() != DECLARATION_DIGEST {
        return Err(Failure::new("fixture-declaration-digest"));
    }
    let declaration: Declaration = str::from_utf8(&declaration_bytes)
        .map_err(|error| Failure::from_error("fixture-declaration", error))?
        .parse()
        .map_err(|error| Failure::from_error("fixture-declaration", error))?;
    if declaration.id != COLLECTION {
        return Err(Failure::new("fixture-declaration-identity"));
    }
    let binding_path = serde_json::to_string(&root.to_string_lossy())
        .map_err(|error| Failure::from_error("synthetic-binding", error))?;
    let bindings: Bindings = format!("synthetic_root = {binding_path}\n")
        .parse()
        .map_err(|error| Failure::from_error("synthetic-binding", error))?;
    let resolved_manifest = declaration
        .manifest_paths(&bindings)
        .map_err(|error| Failure::from_error("fixture-manifest", error))?;
    let [(_, manifest_path)] = resolved_manifest.as_slice() else {
        return Err(Failure::new("fixture-manifest-count"));
    };
    let manifest_bytes =
        fs::read(manifest_path).map_err(|error| Failure::from_error("fixture-manifest", error))?;
    if Digest::of(&manifest_bytes).as_str() != MANIFEST_DIGEST {
        return Err(Failure::new("fixture-manifest-digest"));
    }
    let corpus_root = manifest_path
        .parent()
        .ok_or_else(|| Failure::new("fixture-manifest"))?;
    let entries = parse_entries(&manifest_bytes)?;
    let corpus_digests = verify_fixture_entries(&entries, corpus_root)?;
    let suite_path = declaration.evals.suite.under(&root).join("synthetic.jsonl");
    let suite_bytes =
        fs::read(suite_path).map_err(|error| Failure::from_error("fixture-suite", error))?;
    if Digest::of(&suite_bytes).as_str() != SUITE_DIGEST {
        return Err(Failure::new("fixture-suite-digest"));
    }
    let suite: Suite = str::from_utf8(&suite_bytes)
        .map_err(|error| Failure::from_error("fixture-suite", error))?
        .parse()
        .map_err(|error| Failure::from_error("fixture-suite", error))?;
    if suite.questions.len() != 56
        || suite
            .questions
            .iter()
            .filter(|question| question.answerable)
            .count()
            != 48
    {
        return Err(Failure::new("fixture-suite-accounting"));
    }
    Ok(FixtureInputs {
        root,
        declaration,
        bindings,
        declaration_digest: Digest::of(&declaration_bytes).as_str().to_owned(),
        manifest_digest: Digest::of(&manifest_bytes).as_str().to_owned(),
        corpus_digests,
        suite,
    })
}

fn fixture_root() -> Result<PathBuf, Failure> {
    let Some(root) = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2) else {
        return Err(Failure::new("workspace-root"));
    };
    Ok(root.join("tests/fixtures/synthetic"))
}

pub(super) fn parse_entries(bytes: &[u8]) -> Result<Vec<Entry>, Failure> {
    let manifest =
        str::from_utf8(bytes).map_err(|error| Failure::from_error("fixture-manifest", error))?;
    manifest
        .lines()
        .map(|line| {
            line.parse()
                .map_err(|error| Failure::from_error("fixture-manifest-entry", error))
        })
        .collect()
}

pub(super) fn verify_fixture_entries(
    entries: &[Entry],
    corpus: &Path,
) -> Result<Vec<CorpusDigest>, Failure> {
    if entries.len() != 28 {
        return Err(Failure::new("fixture-entry-count"));
    }
    entries
        .iter()
        .map(|entry| {
            let bytes = fs::read(entry.path.under(corpus))
                .map_err(|error| Failure::from_error("fixture-corpus-entry", error))?;
            let found = Digest::of(&bytes);
            if found != entry.sha256 || u64::try_from(bytes.len()).ok() != Some(entry.bytes.get()) {
                return Err(Failure::new("fixture-corpus-digest"));
            }
            Ok(CorpusDigest {
                path: entry.path.as_str().to_owned(),
                digest: found.as_str().to_owned(),
                bytes: entry.bytes.get(),
            })
        })
        .collect()
}

#[derive(Debug)]
pub(super) struct Scratch {
    pub(super) path: PathBuf,
}

impl Scratch {
    pub(super) fn new() -> Result<Self, Failure> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = env::temp_dir().join(format!(
                "maestro-synthetic-gate-{}-{}",
                process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(Failure::from_error("scratch-directory", error)),
            }
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}
