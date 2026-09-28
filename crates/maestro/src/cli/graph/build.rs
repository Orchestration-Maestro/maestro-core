//! Resumable rule builds backed by leased kernel receipts.

use super::super::{collection, output::Output};
use crate::{failure::Failure, kernel::Kernel};
use maestro_kernel::{
    document::Revision,
    evidence::Span,
    facts::{self, Budget, BuildPlan, ClaimRecord, ClaimSetRecord, Provenance, ReviewState},
};
use maestro_knowledge::{
    graph::rules::{Extractor, Rejection, TableRule, resolve},
    quality,
};
use serde::Serialize;
use std::{fs, num::NonZeroUsize, path::PathBuf, process::ExitCode};
use ulid::Ulid;

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
    /// Total rejections, including those beyond the retention cap.
    rejected: usize,
    /// What it rejected.
    rejections: &'b [Rejection],
}

/// What `knowledge graph build` prints under `--json`.
#[derive(Debug, Serialize)]
struct BuildDocument<'a> {
    /// [`SCHEMA`].
    schema: &'static str,
    /// Build job identity.
    job: String,
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
    /// Total candidates rejected.
    rejected: usize,
    /// Number of retained rejection receipts.
    retained_rejections: usize,
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
    arguments: &Arguments,
) -> Result<ExitCode, Failure> {
    let collection = &arguments.collection;
    let rule = &arguments.rule;
    let text = fs::read_to_string(rule).map_err(|error| {
        Failure::refused(format!(
            "the rule {} cannot be read: {error}",
            rule.display()
        ))
    })?;
    let rule = TableRule::parse(&text).map_err(|error| Failure::refused_by(&error))?;
    collection::declared(kernel, collection)?;
    let revisions = sources(kernel, collection, &rule)?;
    let provenance = rule.provenance();
    let plan = BuildPlan {
        collection_id: collection.clone(),
        provenance: provenance.clone(),
        sources: revisions
            .iter()
            .map(|revision| revision.id.clone())
            .collect(),
        budget: Budget {
            max_claims: arguments.max_claims,
            max_rejections: arguments.max_retained_rejections,
        },
    };
    let work = super::job::Work {
        plan: &plan,
        revisions: &revisions,
        extractor: &rule,
    };
    let result = super::job::run(kernel, output, &work)?;
    let rejections: Vec<_> = result
        .record
        .rejections
        .iter()
        .map(|rejection| Rejection {
            revision_id: rejection.revision_id.clone(),
            block_id: rejection.block_id.clone(),
            reason: rejection.reason.clone(),
        })
        .collect();
    let built = Built {
        collection,
        provenance: &provenance,
        revisions: &revisions,
        rejected: result.record.rejected(),
        rejections: &rejections,
    };
    let document = built.document(result.job, result.set.as_ref());
    if result.set.is_none() {
        output.refusal(
            &document,
            &format!(
                "the rule admitted no claim: {} rejected ({} retained)\n{}",
                built.rejected,
                rejections.len(),
                rejected(&rejections)
            ),
        )?;
        return Ok(ExitCode::from(2));
    }
    if let Some(generation) = arguments.generation {
        super::attach::attach(kernel, output, result.job, generation)?;
    }
    output.result(&document, &summary(&document))?;
    Ok(ExitCode::SUCCESS)
}

/// Frozen build inputs and optional independent attachment target.
#[derive(Debug, clap::Args)]
pub(in crate::cli) struct Arguments {
    /// The collection's declared ID.
    #[arg(long)]
    pub(in crate::cli) collection: String,
    /// Strict standalone table rule.
    #[arg(long, value_name = "PATH")]
    pub(in crate::cli) rule: PathBuf,
    /// Attach the completed build to this unpublished generation.
    #[arg(long)]
    pub(in crate::cli) generation: Option<i64>,
    /// Maximum accepted claims; changing this changes the frozen plan.
    #[arg(long, default_value_t = 1_000_000, value_parser = positive_claim_budget)]
    pub(in crate::cli) max_claims: usize,
    /// Maximum retained rejection receipts; total rejections are still counted.
    #[arg(long, default_value_t = 1_000_000)]
    pub(in crate::cli) max_retained_rejections: usize,
}

/// Parse a nonzero, platform-sized claim budget at the argument boundary.
fn positive_claim_budget(value: &str) -> Result<usize, String> {
    value
        .parse::<NonZeroUsize>()
        .map(NonZeroUsize::get)
        .map_err(|_| format!("expected a value in range 1..={}", usize::MAX))
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

impl Built<'_> {
    /// The document it prints: with the admitted set `record`, its id, its
    /// claims and how their subjects resolve, when there is one.
    fn document<'d>(&'d self, job: Ulid, record: Option<&'d ClaimSetRecord>) -> BuildDocument<'d> {
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
            job: job.to_string(),
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
            rejected: self.rejected,
            retained_rejections: self.rejections.len(),
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
        "admitted {} claims as the claim set {}; {} rejected ({} retained)",
        document.claims.len(),
        document.claim_set.unwrap_or_default(),
        document.rejected,
        document.retained_rejections
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
