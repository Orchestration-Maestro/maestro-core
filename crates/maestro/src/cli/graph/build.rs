//! `knowledge graph build`: a collection's claims built with one strict
//! table rule (FR-S2-004) and admitted through the kernel's one write path,
//! [`Database::record_claim_set`](maestro_kernel::store::Database::record_claim_set).
//! The rule reads the collection's eligible revisions, as the quality gate
//! defines them ([`quality::eligible`]), whose original has its source
//! digest. The build runs it through the [`Extractor`] port, and runs no
//! model, script or graph service. Its claims are admitted as one frozen,
//! ordered set, or none; a rebuild of the same inputs finds the same set and
//! records nothing more, and a changed rule is another profile, so another
//! set. What it rejects is printed with its reason. Under `--json` it prints
//! `maestro-cli/knowledge-graph-build/1`, its `schema` first; a build that
//! admits no claim prints its rejections and exits 2.

use super::super::{collection, output::Output};
use crate::{
    failure::{Failure, chain},
    kernel::Kernel,
};
use maestro_kernel::{
    document::Revision,
    evidence::Span,
    facts::{self, ClaimRecord, ClaimSet, ClaimSetRecord, Provenance, ReviewState},
};
use maestro_knowledge::{
    graph::{
        rules::{Extraction, Extractor, Rejection, TableRule, resolve},
        verify::{Source, SourceError},
    },
    quality,
};
use serde::Serialize;
use std::{fs, path::Path, process::ExitCode};

/// The schema of the document it prints under `--json`.
const SCHEMA: &str = "maestro-cli/knowledge-graph-build/1";

/// What a build read and rejected, for the document it prints.
struct Built<'b> {
    /// The collection.
    collection: &'b str,
    /// What extracted its claims.
    provenance: &'b Provenance,
    /// The revisions it read.
    revisions: &'b [Revision],
    /// What it rejected.
    rejections: &'b [Rejection],
}

/// What `knowledge graph build` prints under `--json`.
#[derive(Debug, Serialize)]
struct BuildDocument<'a> {
    /// [`SCHEMA`].
    schema: &'static str,
    /// The collection's ID.
    collection: &'a str,
    /// What extracted the claims.
    extractor: ExtractorDocument<'a>,
    /// The revisions it read, ordered by document.
    revisions: Vec<&'a str>,
    /// The admitted claim set's ID; none when it admitted nothing.
    claim_set: Option<&'a str>,
    /// The admitted claims, in the set's order.
    claims: Vec<ClaimDocument<'a>>,
    /// How the admitted claims' subjects resolve.
    entities: Vec<EntityDocument>,
    /// What it rejected, and why.
    rejections: &'a [Rejection],
}

/// An extractor, as the document prints it.
#[derive(Debug, Serialize)]
struct ExtractorDocument<'a> {
    /// Its name: the rule's ID.
    id: &'a str,
    /// The digest of its rule or profile.
    profile: &'a str,
}

/// An admitted claim, as the document prints it.
#[derive(Debug, Serialize)]
struct ClaimDocument<'a> {
    /// Its ID.
    id: &'a str,
    /// What it is about.
    subject: NameDocument<'a>,
    /// What it says.
    predicate: &'static str,
    /// Its literal or collection-local entity endpoint.
    object: ObjectDocument<'a>,
    /// What extracted it.
    extractor: &'a str,
    /// The digest of its extractor's rule or profile.
    profile: &'a str,
    /// Its review state.
    review: &'static str,
    /// Its supports.
    supports: Vec<SupportDocument<'a>>,
}

/// An entity's kind and exact spelling.
#[derive(Debug, Serialize)]
struct NameDocument<'a> {
    /// Its kind.
    kind: &'a str,
    /// Its name, spelled as the source spells it.
    name: &'a str,
}

/// A claim endpoint, retaining the original literal JSON representation.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(super) enum ObjectDocument<'a> {
    /// A typed literal.
    Literal {
        /// Its type.
        #[serde(rename = "type")]
        kind: &'static str,
        /// Its unchanged source spelling.
        lexeme: &'a str,
    },
    /// An entity in the claim's collection.
    Entity {
        /// Its closed kind.
        kind: &'static str,
        /// Its unchanged source name.
        name: &'a str,
    },
}

impl<'a> From<&'a facts::Object> for ObjectDocument<'a> {
    fn from(object: &'a facts::Object) -> Self {
        match object {
            facts::Object::Literal(literal) => Self::Literal {
                kind: literal.kind.as_str(),
                lexeme: &literal.lexeme,
            },
            facts::Object::Entity(entity) => Self::Entity {
                kind: entity.kind.as_str(),
                name: &entity.name,
            },
        }
    }
}

/// A claim's support.
#[derive(Debug, Serialize)]
struct SupportDocument<'a> {
    /// The revision it quotes.
    revision_id: &'a str,
    /// The canonical block that holds it.
    block_id: &'a str,
    /// Its half-open byte span in the original.
    span: Span,
    /// The SHA-256 of the quoted bytes.
    quote_sha256: &'a str,
}

/// How a subject resolves.
#[derive(Debug, Serialize)]
struct EntityDocument {
    /// Its kind.
    kind: String,
    /// Its exact spelling.
    name: String,
    /// Its name without accents or case.
    normalized: String,
    /// The subjects whose name normalizes alike.
    colliding: Vec<ColliderDocument>,
}

/// A subject that collides with another.
#[derive(Debug, Serialize)]
struct ColliderDocument {
    /// Its kind.
    kind: String,
    /// Its exact spelling.
    name: String,
}

/// Builds the claims of the collection `collection` with the table rule in
/// the file `rule`, admits them, and prints what it admitted and rejected.
///
/// # Errors
///
/// [`Failure::Refused`] when the rule file cannot be read or is not a strict
/// table rule, the collection was not added, no eligible revision has the
/// rule's source digest, or the kernel refuses the claims;
/// [`Failure::Failed`] when the kernel fails.
pub(in crate::cli) fn run(
    kernel: &Kernel,
    output: Output,
    collection: &str,
    rule: &Path,
) -> Result<ExitCode, Failure> {
    let text = fs::read_to_string(rule).map_err(|error| {
        Failure::refused(format!(
            "the rule {} cannot be read: {error}",
            rule.display()
        ))
    })?;
    let rule = TableRule::parse(&text).map_err(|error| Failure::refused_by(&error))?;
    collection::declared(kernel, collection)?;
    let revisions = sources(kernel, collection, &rule)?;
    let extractor: &dyn Extractor = &rule;
    let extraction = extract(kernel, extractor, &revisions)?;
    let provenance = extractor.provenance();
    let built = Built {
        collection,
        provenance: &provenance,
        revisions: &revisions,
        rejections: &extraction.rejections,
    };
    if extraction.claims.is_empty() {
        let document = built.document(None);
        let diagnostic = format!(
            "the rule admitted no claim: {} rejected\n{}",
            extraction.rejections.len(),
            rejected(document.rejections)
        );
        output.refusal(&document, &diagnostic)?;
        return Ok(ExitCode::from(2));
    }
    let set = ClaimSet {
        collection_id: collection.to_owned(),
        claims: extraction.claims,
    };
    let record = kernel
        .database
        .record_claim_set(&kernel.scopes, &set)
        .map_err(|error| claim_failure(&error))?;
    let document = built.document(Some(&record));
    output.result(&document, &summary(&document))?;
    Ok(ExitCode::SUCCESS)
}

/// What `extractor` finds in `revisions`: the claims of each, in order, and
/// what it rejected, a revision whose artifacts are not what it says among
/// them.
///
/// # Errors
///
/// [`Failure::Failed`] when the artifact store fails.
fn extract(
    kernel: &Kernel,
    extractor: &dyn Extractor,
    revisions: &[Revision],
) -> Result<Extraction, Failure> {
    let mut extraction = Extraction::default();
    for revision in revisions {
        match Source::read(&kernel.database, revision) {
            Ok(source) => {
                let found = extractor.extract(&source);
                extraction.claims.extend(found.claims);
                extraction.rejections.extend(found.rejections);
            }
            Err(SourceError::Store(error)) => return Err(Failure::failed_by(&error)),
            Err(error) => extraction.rejections.push(rejection(revision, &error)),
        }
    }
    Ok(extraction)
}

/// The eligible revisions of `collection`, as the quality gate defines
/// them, whose original has `rule`'s source digest, ordered by document.
///
/// # Errors
///
/// [`Failure::Refused`] when there is none, [`Failure::Failed`] when the
/// kernel fails.
fn sources(kernel: &Kernel, collection: &str, rule: &TableRule) -> Result<Vec<Revision>, Failure> {
    let mut eligible: Vec<Revision> =
        quality::eligible(&kernel.database, &kernel.scopes, collection)
            .map_err(|error| Failure::failed_by(&error))?
            .into_iter()
            .filter(|revision| revision.original_digest == *rule.source_sha256())
            .collect();
    if eligible.is_empty() {
        return Err(Failure::refused(format!(
            "no eligible revision of the collection {collection} has the rule's source digest \
             sha256:{}",
            rule.source_sha256().as_str()
        )));
    }
    eligible.sort_by(|left, right| left.document_id.cmp(&right.document_id));
    Ok(eligible)
}

/// The rejection of `revision`, whose artifacts are not what it says.
fn rejection(revision: &Revision, error: &SourceError) -> Rejection {
    Rejection {
        revision_id: revision.id.clone(),
        block_id: None,
        reason: chain(error),
    }
}

/// A refusal for a claim set the kernel refused as input, exit 2; a
/// failure, exit 1, when its store failed or a stored original no longer
/// holds what knowledge located in it.
pub(super) fn claim_failure(error: &facts::Error) -> Failure {
    match error {
        facts::Error::Store(_)
        | facts::Error::Job(_)
        | facts::Error::DigestMismatch { .. }
        | facts::Error::SpanOutOfRange { .. }
        | facts::Error::SpanOffBoundary { .. }
        | facts::Error::QuoteMismatch { .. } => Failure::failed_by(error),
        facts::Error::Unauthorized
        | facts::Error::Invalid(_)
        | facts::Error::UnknownBuild(_)
        | facts::Error::Unfinished { .. }
        | facts::Error::OverBudget { .. }
        | facts::Error::Conflict(_)
        | facts::Error::UnknownRevision { .. }
        | facts::Error::IneligibleRevision { .. } => Failure::refused_by(error),
    }
}

impl Built<'_> {
    /// The document it prints: with the admitted set `record`, its id, its
    /// claims and how their subjects resolve, when there is one.
    fn document<'d>(&'d self, record: Option<&'d ClaimSetRecord>) -> BuildDocument<'d> {
        let claims = record.map_or(&[][..], |record| record.claims.as_slice());
        let subjects: Vec<_> = claims
            .iter()
            .map(|claim| claim.claim.subject.clone())
            .collect();
        let entities = resolve(&subjects)
            .into_iter()
            .map(|resolution| EntityDocument {
                kind: resolution.subject.kind.as_str().to_owned(),
                name: resolution.subject.name,
                normalized: resolution.normalized,
                colliding: resolution
                    .colliding
                    .into_iter()
                    .map(|other| ColliderDocument {
                        kind: other.kind.as_str().to_owned(),
                        name: other.name,
                    })
                    .collect(),
            })
            .collect();
        BuildDocument {
            schema: SCHEMA,
            collection: self.collection,
            extractor: ExtractorDocument {
                id: &self.provenance.extractor,
                profile: self.provenance.profile.as_str(),
            },
            revisions: self
                .revisions
                .iter()
                .map(|revision| revision.id.as_str())
                .collect(),
            claim_set: record.map(|record| record.id.as_str()),
            claims: claims.iter().map(claim).collect(),
            entities,
            rejections: self.rejections,
        }
    }
}

/// `record` as the document prints it.
fn claim(record: &ClaimRecord) -> ClaimDocument<'_> {
    let claim = &record.claim;
    ClaimDocument {
        id: record.id.as_str(),
        subject: NameDocument {
            kind: claim.subject.kind.as_str(),
            name: &claim.subject.name,
        },
        predicate: claim.predicate.as_str(),
        object: ObjectDocument::from(&claim.object),
        extractor: &claim.provenance.extractor,
        profile: claim.provenance.profile.as_str(),
        review: review(record.review),
        supports: claim
            .supports
            .iter()
            .map(|support| SupportDocument {
                revision_id: &support.revision_id,
                block_id: &support.block_id,
                span: support.span,
                quote_sha256: support.quote_digest.as_str(),
            })
            .collect(),
    }
}

/// The name of `state`.
fn review(state: ReviewState) -> &'static str {
    match state {
        ReviewState::Unreviewed => "unreviewed",
        ReviewState::Accepted => "accepted",
        ReviewState::Rejected => "rejected",
        ReviewState::Flagged => "flagged",
    }
}

/// `rejections` for people, one line each.
fn rejected(rejections: &[Rejection]) -> String {
    let lines: Vec<String> = rejections
        .iter()
        .map(|rejection| {
            format!(
                "rejected {} {}: {}",
                rejection.revision_id,
                rejection.block_id.as_deref().unwrap_or("-"),
                rejection.reason
            )
        })
        .collect();
    lines.join("\n")
}

/// `document`, admitted, for people: the set, its claims, then what was
/// rejected.
fn summary(document: &BuildDocument<'_>) -> String {
    let mut lines = vec![format!(
        "admitted {} claims as the claim set {}; {} rejected",
        document.claims.len(),
        document.claim_set.unwrap_or_default(),
        document.rejections.len()
    )];
    for claim in &document.claims {
        let (kind, name) = match &claim.object {
            ObjectDocument::Literal { kind, lexeme } => (kind, lexeme),
            ObjectDocument::Entity { kind, name } => (kind, name),
        };
        lines.push(format!(
            "{} {} {} {}",
            claim.subject.name, claim.predicate, kind, name
        ));
    }
    let rejected = rejected(document.rejections);
    if !rejected.is_empty() {
        lines.push(rejected);
    }
    lines.join("\n")
}
