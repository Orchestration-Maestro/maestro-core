//! Immutable source-backed identity review snapshots over frozen claim sets.
//! A snapshot names all its sources, including history; revoking any collection
//! hides the entire snapshot rather than leaking a cross-collection alias.

use super::{
    error::Error,
    read::load_set,
    types::{ClaimRecord, Object, ReviewState},
    write::authorize,
};
use crate::{artifact::Digest, scope::ScopeSet, store::Database};
use rusqlite::{Connection, OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Which sourced entity a mention names. Literals are never endpoints here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Endpoint {
    /// The claim's subject.
    Subject,
    /// The claim's entity-valued object.
    Object,
}

/// An occurrence backed by every verified support of an immutable claim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mention {
    /// The supported claim's application identity.
    pub claim: Digest,
    /// Its named endpoint, never a literal value.
    pub endpoint: Endpoint,
}

/// Explicit review, not an extracted relation or a truth guarantee.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionKind {
    /// Resolve the left mention to the right identity.
    Alias,
    /// Reverse an alias; retain the distinct identity and the old decision.
    Separate,
    /// The right claim supersedes the left, without deleting either.
    Supersedes,
}

/// A sourced review action. Repeating a pair with a new action reverses it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    /// The proposed alias or superseded claim's subject.
    pub left: Mention,
    /// Its reviewed target or replacement claim's subject.
    pub right: Mention,
    /// The review disposition.
    pub kind: DecisionKind,
    /// The nonempty review rationale, retained with the source references.
    pub reason: String,
}

/// Frozen sources and new decisions for one resolution snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionInput {
    /// Version of the normalization and grouping algorithm used to derive entities.
    pub resolver_version: String,
    /// Frozen sets, including every set the previous snapshot used.
    pub sets: Vec<Digest>,
    /// The snapshot this one extends; it remains unchanged.
    pub previous: Option<Digest>,
    /// New decisions in review order; old decisions are retained automatically.
    pub decisions: Vec<Decision>,
}

/// An attributed immutable review record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRecord {
    /// The principal that submitted the review.
    pub reviewer: String,
    /// Its source-backed action.
    pub decision: Decision,
}

/// Authority returned only after refreshing the reader's grants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionSnapshot {
    /// Frozen normalization and grouping algorithm identity.
    pub resolver_version: String,
    /// Content-addressed pin, independent of record time.
    pub id: Digest,
    /// Previous immutable pin, when this snapshot extends one.
    pub previous: Option<Digest>,
    /// The ordered, deduplicated frozen source sets.
    pub sets: Vec<Digest>,
    /// Every immutable source claim, including contradictions and old versions.
    pub claims: Vec<ClaimRecord>,
    /// All decisions, oldest first, with their original reviewer.
    pub history: Vec<ReviewRecord>,
}

/// Stored payload; source claims remain in their original authority tables.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    /// Algorithm identity is part of the immutable snapshot digest.
    resolver_version: String,
    /// Previous snapshot, retained as history, never replaced.
    previous: Option<Digest>,
    /// All sets needed to authorize sources and history.
    sets: Vec<Digest>,
    /// Mutable claim reviews frozen by claim identity at snapshot creation.
    reviews: BTreeMap<Digest, ReviewState>,
    /// Materialized history avoids recursive reads of an unbounded parent chain.
    history: Vec<ReviewRecord>,
}

impl Database {
    /// Record a sourced snapshot, replaying identical input idempotently.
    /// Requires request coverage and current grants for every source. Reviews
    /// cannot add names, sources or literal nodes absent from the claims.
    /// The validator checks the complete proposed snapshot inside the write
    /// transaction, before insertion. It must perform no I/O or reentrant writes.
    ///
    /// # Errors
    /// Returns [`Error::Unauthorized`] for inaccessible inputs, [`Error::Invalid`]
    /// for unsourced decisions, the validator's typed refusal, or [`Error::Store`]
    /// for storage failures.
    pub fn record_resolution(
        &self,
        request: &ScopeSet,
        principal: &str,
        input: &ResolutionInput,
        validate: &dyn Fn(&ResolutionSnapshot) -> Result<(), Error>,
    ) -> Result<ResolutionSnapshot, Error> {
        self.write(|transaction| {
            // Refresh after acquiring the writer: a revocation cannot commit mid-write.
            let scopes = self.visible(principal)?;
            let distinct: BTreeSet<_> = input.sets.iter().cloned().collect();
            let sets: Vec<_> = distinct.into_iter().collect();
            if principal.is_empty() || sets.is_empty() || input.resolver_version.trim().is_empty() {
                return Err(invalid(
                    "a resolution needs a reviewer, resolver version and frozen sources",
                ));
            }
            let claims = source_claims(transaction, &scopes, &sets)?.ok_or(Error::Unauthorized)?;
            for claim in &claims {
                authorize(request, &claim.collection_id)?;
            }
            let mut history = previous_history(transaction, &scopes, input, &sets)?;
            for decision in &input.decisions {
                check_decision(&claims, decision)?;
                history.push(ReviewRecord {
                    reviewer: principal.to_owned(),
                    decision: decision.clone(),
                });
            }
            let stored = Stored {
                resolver_version: input.resolver_version.clone(),
                previous: input.previous.clone(),
                sets,
                reviews: claims
                    .iter()
                    .map(|claim| (claim.id.clone(), claim.review))
                    .collect(),
                history,
            };
            let body = serde_json::to_string(&stored)
                .map_err(|_| invalid("invalid resolution payload"))?;
            let id = Digest::of(format!("maestro-resolution/1:{body}").as_bytes());
            let proposed = snapshot(id.clone(), stored, claims)?;
            validate(&proposed)?;
            transaction.execute(
                "INSERT INTO graph_resolutions (id, previous_id, reviewer, body)
                 SELECT ?1, ?2, ?3, ?4
                 WHERE NOT EXISTS (SELECT 1 FROM graph_resolutions WHERE id = ?1)",
                params![
                    id.as_str(),
                    input.previous.as_ref().map(Digest::as_str),
                    principal,
                    body
                ],
            )?;
            Ok(proposed)
        })
    }

    /// Read within request scopes and current grants; hidden and unknown pins
    /// both return none. A stale request never restores a revoked grant.
    ///
    /// # Errors
    /// Returns [`Error::Store`] for storage failures or [`Error::Invalid`] for
    /// a corrupt stored payload.
    pub fn resolution(
        &self,
        scopes: &ScopeSet,
        principal: &str,
        id: &Digest,
    ) -> Result<Option<ResolutionSnapshot>, Error> {
        let current = self.visible(principal)?;
        let snapshot = load(&self.reader()?, &current, id)?;
        Ok(snapshot.filter(|snapshot| {
            snapshot
                .claims
                .iter()
                .all(|claim| authorize(scopes, &claim.collection_id).is_ok())
        }))
    }
}

/// Hydrate only after every frozen collection is authorized.
fn load(
    connection: &Connection,
    scopes: &ScopeSet,
    id: &Digest,
) -> Result<Option<ResolutionSnapshot>, Error> {
    let body: Option<String> = connection
        .query_row(
            "SELECT body FROM graph_resolutions WHERE id = ?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    let Some(body) = body else { return Ok(None) };
    let stored: Stored =
        serde_json::from_str(&body).map_err(|_| invalid("invalid resolution payload"))?;
    source_claims(connection, scopes, &stored.sets)?
        .map(|claims| snapshot(id.clone(), stored, claims))
        .transpose()
}

/// Unique claims, in ID order, without collapsing conflicting objects or validity.
fn source_claims(
    connection: &Connection,
    scopes: &ScopeSet,
    sets: &[Digest],
) -> Result<Option<Vec<ClaimRecord>>, Error> {
    let mut claims = Vec::new();
    for id in sets {
        let Some(set) = load_set(connection, scopes, id)? else {
            return Ok(None);
        };
        claims.extend(set.claims);
    }
    claims.sort_by(|left, right| left.id.cmp(&right.id));
    claims.dedup_by(|left, right| left.id == right.id);
    Ok(Some(claims))
}

/// Keep snapshot assembly identical on write and read.
fn snapshot(
    id: Digest,
    stored: Stored,
    mut claims: Vec<ClaimRecord>,
) -> Result<ResolutionSnapshot, Error> {
    for claim in &mut claims {
        claim.review = *stored
            .reviews
            .get(&claim.id)
            .ok_or_else(|| invalid("missing frozen review"))?;
    }
    Ok(ResolutionSnapshot {
        resolver_version: stored.resolver_version,
        id,
        previous: stored.previous,
        sets: stored.sets,
        claims,
        history: stored.history,
    })
}

/// Verify that both identity endpoints are actually supported entities.
fn check_decision(claims: &[ClaimRecord], decision: &Decision) -> Result<(), Error> {
    if decision.reason.trim().is_empty() {
        return Err(invalid("a review needs a rationale"));
    }
    for mention in [&decision.left, &decision.right] {
        let claim = claims
            .iter()
            .find(|claim| claim.id == mention.claim)
            .ok_or_else(|| invalid("an endpoint has no source claim"))?;
        if mention.endpoint == Endpoint::Object && !matches!(claim.claim.object, Object::Entity(_))
        {
            return Err(invalid("a literal is not an identity endpoint"));
        }
    }
    if decision.kind == DecisionKind::Supersedes
        && (decision.left.endpoint != Endpoint::Subject
            || decision.right.endpoint != Endpoint::Subject
            || decision.left.claim == decision.right.claim)
    {
        return Err(invalid("supersession needs two distinct claim subjects"));
    }
    Ok(())
}

/// A fixed diagnostic without source text.
fn invalid(message: &str) -> Error {
    Error::Invalid(message.to_owned())
}

/// Retain every previous source before copying its attributed review history.
fn previous_history(
    connection: &Connection,
    scopes: &ScopeSet,
    input: &ResolutionInput,
    sets: &[Digest],
) -> Result<Vec<ReviewRecord>, Error> {
    let Some(previous) = &input.previous else {
        return Ok(Vec::new());
    };
    let parent = load(connection, scopes, previous)?.ok_or(Error::Unauthorized)?;
    if !parent.sets.iter().all(|set| sets.contains(set)) {
        return Err(invalid("resolution history cannot discard its sources"));
    }
    Ok(parent.history)
}
