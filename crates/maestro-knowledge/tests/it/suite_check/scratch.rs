//! What the check's tests share: a scratch directory holding a corpus, its
//! manifest and a directory of suites, the documents of a small corpus, and
//! the manifest lines, names and questions the tests write.

use super::check::{Checked, check};
use maestro_kernel::artifact::Digest;
use serde_json::{Value, json};
use std::{
    env, fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// A document whose sections have, in order, the heading paths `Rotation`,
/// `Rotation › By size`, `Rotation › Example`, `Rotation › By time` and
/// `Rotation › Example`.
pub(super) const ROTATION: &str = "# Rotation\n\n## By size\n\nAt 100 MiB.\n\n## Example\n\n\
    Rotate at 50 MiB.\n\n## By time\n\nEvery day.\n\n## Example\n\nRotate daily.\n";
/// A document without a heading, so without a section.
pub(super) const NOTES: &str = "Logs rotate every day.\n\n**Cause**\n\nThe disk fills up.\n";
/// Another document without a section.
pub(super) const LOGS: &str =
    "Old logs are compressed.\n\n**Note**\n\nCompressed logs count toward the limit.\n";
/// The `source_ref` of [`ROTATION`].
pub(super) const ROTATION_REF: &str = "https://handbook.example.org/rotation";
/// The `source_ref` of [`NOTES`].
pub(super) const NOTES_REF: &str = "https://handbook.example.org/notes";
/// The `source_ref` of [`LOGS`].
pub(super) const LOGS_REF: &str = "https://handbook.example.org/logs";

/// A new empty directory under the platform's temporary directory, removed
/// with everything in it when dropped: `corpus/` holds the documents and
/// their manifest, `maestro-corpus.jsonl`, and `evals/` the suites.
pub(super) struct Scratch(PathBuf);

impl Scratch {
    /// A directory of its own for one test.
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-knowledge-suite-check-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("corpus")).unwrap();
        fs::create_dir_all(path.join("evals")).unwrap();
        Self(path)
    }

    /// Writes the document `name` of the corpus.
    pub(super) fn document(&self, name: &str, bytes: &[u8]) {
        fs::write(self.0.join("corpus").join(name), bytes).unwrap();
    }

    /// Writes the corpus manifest, one line of `lines` each.
    pub(super) fn manifest(&self, lines: &[Value]) {
        let text: String = lines.iter().map(|line| line.to_string() + "\n").collect();
        self.document("maestro-corpus.jsonl", text.as_bytes());
    }

    /// Writes the file `name` of the suite directory.
    pub(super) fn evals_file(&self, name: &str, text: &str) {
        fs::write(self.0.join("evals").join(name), text).unwrap();
    }

    /// Writes the suite `name`, one question of `lines` each.
    pub(super) fn suite(&self, name: &str, lines: &[Value]) {
        let text: String = lines.iter().map(|line| line.to_string() + "\n").collect();
        self.evals_file(&format!("{name}.jsonl"), &text);
    }

    /// What the check finds in the suite directory against the manifest.
    pub(super) fn check(&self) -> Checked {
        let manifest = self.0.join("corpus").join("maestro-corpus.jsonl");
        check(&self.0.join("evals"), &manifest)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// A manifest line that declares the document `name` as holding `bytes`,
/// from `source_ref`.
pub(super) fn line(name: &str, bytes: &[u8], source_ref: &str) -> Value {
    json!({
        "schema": "maestro-corpus/1",
        "path": name,
        "sha256": Digest::of(bytes).as_str(),
        "bytes": bytes.len(),
        "source_ref": source_ref,
        "title": name,
        "source_kind": "guide",
    })
}

/// The name of `heading_path` in the document of `source_ref`.
pub(super) fn name(source_ref: &str, heading_path: &[&str]) -> Value {
    json!({"source_ref": source_ref, "heading_path": heading_path})
}

/// The question `id`, which names `expected`, answerable when it names any.
pub(super) fn question(id: &str, expected: &Value) -> Value {
    json!({
        "schema": "maestro-suite/1", "id": id, "language": "en",
        "question": format!("What about {id}?"),
        "answerable": expected != &json!([]), "expected": expected,
    })
}

/// A scratch corpus of [`ROTATION`], [`NOTES`] and [`LOGS`], each declared
/// truly.
pub(super) fn corpus() -> Scratch {
    let scratch = Scratch::new();
    scratch.document("rotation.md", ROTATION.as_bytes());
    scratch.document("notes.md", NOTES.as_bytes());
    scratch.document("logs.md", LOGS.as_bytes());
    scratch.manifest(&[
        line("rotation.md", ROTATION.as_bytes(), ROTATION_REF),
        line("notes.md", NOTES.as_bytes(), NOTES_REF),
        line("logs.md", LOGS.as_bytes(), LOGS_REF),
    ]);
    scratch
}
