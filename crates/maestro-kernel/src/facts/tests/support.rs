//! What the claim tests share: a scratch database holding revisions of a
//! synthetic parameter table with their original Markdown, and claims that
//! quote its rows.

use crate::{
    artifact::Digest,
    document::{Collection, Disposition, Document, Outcome, Revision, RevisionStatus, Source},
    evidence::Span,
    facts::{
        Budget, BuildPlan, Claim, ClaimSet, EntityKind, EntityName, Literal, LiteralKind, Object,
        Predicate, Provenance, Support, Validity,
    },
    job::{Job, Lease, LeaseTiming, NewJob},
    scope::{Right, ScopeSet, collection_path},
    store::{self, Database},
};
use rusqlite::{Connection, params};
use serde_json::{Map, json};
use std::{
    collections::BTreeMap,
    env, fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
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
        populate(Database::open_in(&self.0).unwrap())
    }

    /// [`Scratch::open`], with the migrations numbered below `migration`
    /// alone: the database an older binary left.
    pub(super) fn open_before(&self, migration: &str) -> Database {
        populate(Database::open_before(&self.0, migration).unwrap())
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

/// `database`, once it holds the collections `graph` and `other`, each with
/// the source `docs` and the document `doc-a` or `doc-o`; the first has the
/// accepted revision `rev-a`, the second the accepted revision `rev-o`, both
/// of [`ORIGINAL`].
fn populate(database: Database) -> Database {
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
            kind: EntityKind::Parameter,
            name: name.to_owned(),
        },
        predicate: Predicate::DefaultsTo,
        object: Object::Literal(Literal {
            kind,
            lexeme: lexeme.to_owned(),
        }),
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

/// The claim that the `subject` entity stands in `predicate` to the
/// `object` entity, supported by [`LABEL_ROW`], in the block `block-label`,
/// with no conditions and unknown validity.
pub(super) fn relation_claim(
    subject: (EntityKind, &str),
    predicate: Predicate,
    object: (EntityKind, &str),
) -> Claim {
    let named = |(kind, name): (EntityKind, &str)| EntityName {
        kind,
        name: name.to_owned(),
    };
    Claim {
        subject: named(subject),
        predicate,
        object: Object::Entity(named(object)),
        ..label()
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

/// Seeds the actual 0012 schema, never using the current claim writer. The
/// optional last kind models a legacy row the closed vocabulary cannot admit.
pub(super) fn legacy(scratch: &Scratch, invalid_kind: Option<&str>) -> (Digest, Vec<Digest>) {
    use crate::facts::write::{claim_digest, set_digest};

    let database = scratch.open_before("0013_graph_claim_vocabulary");
    let claims = [label(), retries()];
    let kinds = ["Parameter", invalid_kind.unwrap_or("Parameter")];
    let ids: Vec<_> = claims
        .iter()
        .zip(kinds)
        .map(|(claim, kind)| {
            claim_digest(
                COLLECTION,
                kind,
                claim,
                &claim.supports.iter().collect::<Vec<_>>(),
            )
        })
        .collect();
    let set = set_digest(COLLECTION, &ids);
    database
        .write::<_, store::Error>(|transaction| {
            for ((claim, kind), id) in claims.iter().zip(kinds).zip(&ids) {
                let Object::Literal(literal) = &claim.object else {
                    panic!("legacy fixture must hold literals")
                };
                transaction.execute(
                    "INSERT INTO claims (id, collection_id, subject_kind, subject_name, predicate,
                  object_type, object_lexeme, conditions_json, version_known, world_known,
                  extractor, profile_digest, review_state, support_count, recorded_at)
                 VALUES (?1, ?2, ?3, ?4, 'DEFAULTS_TO', ?5, ?6, '{}', 0, 0,
                  ?7, ?8, 'accepted', 1, '2026-01-02T03:04:05.678Z')",
                    params![
                        id.as_str(),
                        COLLECTION,
                        kind,
                        claim.subject.name,
                        literal.kind.as_str(),
                        literal.lexeme,
                        claim.provenance.extractor,
                        claim.provenance.profile.as_str()
                    ],
                )?;
                for support in &claim.supports {
                    transaction.execute(
                        "INSERT INTO claim_supports VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            id.as_str(),
                            support.revision_id,
                            support.block_id,
                            i64::try_from(support.span.start).unwrap(),
                            i64::try_from(support.span.end).unwrap(),
                            support.quote_digest.as_str()
                        ],
                    )?;
                }
            }
            transaction.execute(
                "INSERT INTO claim_sets VALUES (?1, ?2, 2)",
                params![set.as_str(), COLLECTION],
            )?;
            for (ordinal, id) in ids.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO claim_set_members VALUES (?1, ?2, ?3)",
                    params![set.as_str(), i64::try_from(ordinal).unwrap(), id.as_str()],
                )?;
            }
            Ok(())
        })
        .unwrap();
    (set, ids)
}

/// How long a build's lease lasts in the build tests.
pub(super) const TERM: Duration = Duration::from_secs(60);

/// `seconds` after a fixed instant of the tests' clock.
pub(super) fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_800_000_000 + seconds)
}

/// The provenance every test claim carries.
pub(super) fn provenance() -> Provenance {
    label().provenance
}

/// A build of the collection `graph` from `sources`, in order, with the
/// test claims' provenance and budgets of `max_claims` claims and
/// `max_rejections` kept rejections.
pub(super) fn plan(sources: &[&str], max_claims: usize, max_rejections: usize) -> BuildPlan {
    BuildPlan {
        collection_id: COLLECTION.to_owned(),
        provenance: provenance(),
        sources: sources.iter().map(|&source| source.to_owned()).collect(),
        budget: Budget {
            max_claims,
            max_rejections,
        },
    }
}

/// The lease of a new build job for `plan`, taken by `holder` at `at(0)`;
/// the build is not begun.
pub(super) fn build_job(database: &Database, plan: &BuildPlan, holder: &str) -> Lease {
    let job = submit_build(database, plan);
    database.take_job(job.id, holder, at(0), TERM).unwrap()
}

/// Submit a job using every frozen plan field, without taking its lease.
pub(super) fn submit_build(database: &Database, plan: &BuildPlan) -> Job {
    let scope = collection_path(&plan.collection_id).parse().unwrap();
    let inputs = json!({ "collection": plan.collection_id,
        "extractor": plan.provenance.extractor, "profile": plan.provenance.profile.as_str(),
        "sources": plan.sources, "max_claims": plan.budget.max_claims,
        "max_rejections": plan.budget.max_rejections });
    let new = NewJob {
        kind: "knowledge.graph.build",
        inputs: &inputs,
        scope: &scope,
        resource: None,
    };
    database.submit_job(&new, at(0)).unwrap()
}

/// Records the accepted revision `rev-b` of [`ORIGINAL`] in the new document
/// `doc-b` of the collection `graph`.
pub(super) fn second_revision(database: &Database) {
    database
        .record_document(&Document {
            id: "doc-b".to_owned(),
            collection_id: COLLECTION.to_owned(),
            source_id: "docs".to_owned(),
            source_ref: "corpus-path:doc-b.md".to_owned(),
        })
        .unwrap();
    revise(
        database,
        "doc-b",
        "rev-b",
        RevisionStatus::Valid,
        Some(Outcome::Accepted),
    );
}

/// `claim` with every support quoting the revision `revision`.
pub(super) fn on(revision: &str, mut claim: Claim) -> Claim {
    for support in &mut claim.supports {
        revision.clone_into(&mut support.revision_id);
    }
    claim
}

/// A renewal at the fixture clock's second, for its standard term.
pub(super) fn timing(seconds: u64) -> LeaseTiming {
    LeaseTiming {
        now: at(seconds),
        term: TERM,
    }
}
