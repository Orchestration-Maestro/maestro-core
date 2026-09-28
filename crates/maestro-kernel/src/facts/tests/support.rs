//! What the claim tests share: a scratch database holding revisions of a
//! synthetic parameter table with their original Markdown, and claims that
//! quote its rows.

use crate::{
    artifact::Digest,
    document::{Collection, Disposition, Document, Outcome, Revision, RevisionStatus, Source},
    evidence::Span,
    facts::{
        Claim, ClaimSet, EntityName, Literal, LiteralKind, Predicate, Provenance, Support, Validity,
    },
    scope::{Right, ScopeSet},
    store::{self, Database},
};
use rusqlite::Connection;
use serde_json::Map;
use std::{
    collections::BTreeMap,
    env, fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// The original Markdown of every revision: a table whose first row holds a
/// character of two bytes.
pub(super) const ORIGINAL: &str = "# Lantern controller\n\n## Parameters\n\n\
                                   | Parameter | Type | Default |\n\
                                   | --- | --- | --- |\n\
                                   | label | text | café |\n\
                                   | retries | integer | 3 |\n";

/// The first body row of [`ORIGINAL`].
pub(super) const LABEL_ROW: &str = "| label | text | café |\n";

/// The second body row of [`ORIGINAL`].
pub(super) const RETRIES_ROW: &str = "| retries | integer | 3 |\n";

/// The collection every claim belongs to.
pub(super) const COLLECTION: &str = "graph";

/// A new empty directory under the platform's temporary directory, removed
/// with everything in it when dropped.
pub(super) struct Scratch(pub(super) PathBuf);

impl Scratch {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-kernel-facts-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// The kernel database of this directory, with the collections `graph`
    /// and `other`, each with the source `docs` and the document `doc-a` or
    /// `doc-o`; the first has the accepted revision `rev-a`, the second the
    /// accepted revision `rev-o`, both of [`ORIGINAL`].
    pub(super) fn open(&self) -> Database {
        let database = Database::open_in(&self.0).unwrap();
        for (collection, document, revision) in
            [(COLLECTION, "doc-a", "rev-a"), ("other", "doc-o", "rev-o")]
        {
            database
                .record_collection(&Collection {
                    id: collection.to_owned(),
                    title: format!("The {collection} collection"),
                    visibility: "public".to_owned(),
                    profiles: BTreeMap::new(),
                })
                .unwrap();
            database
                .record_source(&Source {
                    collection_id: collection.to_owned(),
                    id: "docs".to_owned(),
                    kind: "import".to_owned(),
                    transport: None,
                    reference: "corpus_root:docs.jsonl".to_owned(),
                    profiles: BTreeMap::new(),
                })
                .unwrap();
            database
                .record_document(&Document {
                    id: document.to_owned(),
                    collection_id: collection.to_owned(),
                    source_id: "docs".to_owned(),
                    source_ref: format!("corpus-path:{document}.md"),
                })
                .unwrap();
            revise(
                &database,
                document,
                revision,
                RevisionStatus::Valid,
                Some(Outcome::Accepted),
            );
        }
        database
    }

    /// A reader of this directory's database of its own.
    pub(super) fn outside(&self) -> Connection {
        Connection::open(self.0.join("kernel.sqlite3")).unwrap()
    }

    /// Where the artifact store of this directory's database keeps `digest`.
    pub(super) fn stored(&self, digest: &Digest) -> PathBuf {
        let hex = digest.as_str();
        self.0
            .join("artifacts")
            .join("sha256")
            .join(&hex[..2])
            .join(&hex[2..4])
            .join(hex)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

/// Records the revision `id` of `document`, of [`ORIGINAL`], with `status`
/// and, when there is one, the disposition `outcome`.
pub(super) fn revise(
    database: &Database,
    document: &str,
    id: &str,
    status: RevisionStatus,
    outcome: Option<Outcome>,
) {
    let revision = Revision {
        id: id.to_owned(),
        document_id: document.to_owned(),
        original_digest: database.put(ORIGINAL.as_bytes(), "text/markdown").unwrap(),
        canonical_digest: database.put(b"{}", "application/json").unwrap(),
        status,
        captured_at: None,
        metadata: Map::new(),
    };
    database.record_revision(&revision).unwrap();
    if let Some(outcome) = outcome {
        database
            .record_disposition(&Disposition {
                revision_id: id.to_owned(),
                outcome,
                reasons: Vec::new(),
                rule_ids: Vec::new(),
                decided_by: "test".to_owned(),
            })
            .unwrap();
    }
}

/// The set of `principal`, once granted the scope `path` alone.
pub(super) fn granted(database: &Database, principal: &str, path: &str) -> ScopeSet {
    database
        .grant(principal, &path.parse().unwrap(), Right::Read, "test")
        .unwrap();
    database.visible(principal).unwrap()
}

/// The span of [`ORIGINAL`] that `text` takes, where it first appears.
pub(super) fn span_of(text: &str) -> Span {
    let start = ORIGINAL.find(text).unwrap();
    Span {
        start,
        end: start + text.len(),
    }
}

/// The support that quotes `text` of the revision `rev-a`, in the block
/// `block`.
pub(super) fn quoting(text: &str, block: &str) -> Support {
    Support {
        revision_id: "rev-a".to_owned(),
        block_id: block.to_owned(),
        span: span_of(text),
        quote_digest: Digest::of(text.as_bytes()),
    }
}

/// The claim that the parameter `name` defaults to `lexeme` of `kind`,
/// supported by `row`, in the block `block-<name>`, with no conditions and
/// unknown validity.
pub(super) fn default_claim(name: &str, kind: LiteralKind, lexeme: &str, row: &str) -> Claim {
    Claim {
        subject: EntityName {
            kind: "Parameter".to_owned(),
            name: name.to_owned(),
        },
        predicate: Predicate::DefaultsTo,
        object: Literal {
            kind,
            lexeme: lexeme.to_owned(),
        },
        conditions: BTreeMap::new(),
        version: Validity::Unknown,
        world: Validity::Unknown,
        provenance: Provenance {
            extractor: "synthetic-defaults/1".to_owned(),
            profile: Digest::of(b"synthetic-defaults/1"),
        },
        supports: vec![quoting(row, &format!("block-{name}"))],
    }
}

/// The claim that `label` defaults to the text `café`.
pub(super) fn label() -> Claim {
    default_claim("label", LiteralKind::Text, "café", LABEL_ROW)
}

/// The claim that `retries` defaults to the integer `3`.
pub(super) fn retries() -> Claim {
    default_claim("retries", LiteralKind::Integer, "3", RETRIES_ROW)
}

/// The set of `claims` in the collection `graph`.
pub(super) fn set_of(claims: Vec<Claim>) -> ClaimSet {
    ClaimSet {
        collection_id: COLLECTION.to_owned(),
        claims,
    }
}

/// How many rows each claim table holds, in the order claims, supports,
/// sets and members.
pub(super) fn counts(scratch: &Scratch) -> [i64; 4] {
    let reader = scratch.outside();
    [
        "claims",
        "claim_supports",
        "claim_sets",
        "claim_set_members",
    ]
    .map(|table| {
        reader
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    })
}

/// Runs `statement` on the database's writer and commits what it did.
pub(super) fn execute(database: &Database, statement: &str) -> Result<usize, store::Error> {
    database.write(|transaction| {
        transaction
            .execute(statement, [])
            .map_err(store::Error::from)
    })
}
