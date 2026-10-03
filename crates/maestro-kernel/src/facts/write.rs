//! Admitting a claim set: its form checked, every support verified from the
//! authority, then the whole set recorded in one write, or nothing.

use super::{
    error::Error,
    quote::eligible,
    read::load_set,
    types::{Claim, ClaimSet, ClaimSetRecord, Object, Support, Validity},
};
use crate::{
    artifact::Digest,
    scope::{Scope, ScopeSet, check_collection_name, collection_path},
    store::Database,
};
use rusqlite::{OptionalExtension as _, Transaction, params};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

/// A claim ready to record: its id and its content, supports in their
/// canonical order.
struct Admitted<'c> {
    /// The SHA-256 of its canonical form.
    id: Digest,
    /// Its content.
    claim: &'c Claim,
    /// Its supports, ordered by revision, block and span.
    supports: Vec<&'c Support>,
}

impl Database {
    /// Admits `set`: checks the form of every claim, verifies every support
    /// against its revision's original bytes, then records the claims, their
    /// supports and the ordered set in one write. Every claim is recorded
    /// unreviewed. A claim recorded before, by content, is shared; a set
    /// recorded before is returned as it is, and nothing more is recorded.
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`] when `scopes` does not cover the collection,
    /// [`Error::Invalid`] for a claim of the wrong form,
    /// [`Error::UnknownRevision`] for a revision of another collection or one
    /// `scopes` does not cover, [`Error::IneligibleRevision`],
    /// [`Error::DigestMismatch`], [`Error::SpanOutOfRange`],
    /// [`Error::SpanOffBoundary`] and [`Error::QuoteMismatch`] for a support
    /// that fails verification, and [`Error::Store`] when the database or
    /// the artifact store refuses. Nothing is recorded then.
    pub fn record_claim_set(
        &self,
        scopes: &ScopeSet,
        set: &ClaimSet,
    ) -> Result<ClaimSetRecord, Error> {
        let prepared = self.prepare_claims(scopes, set)?;
        self.write(|transaction| prepared.record_set(transaction, scopes))
    }

    /// Validate claim form and original bytes without holding a write transaction.
    pub(super) fn prepare_claims<'c>(
        &self,
        scopes: &ScopeSet,
        set: &'c ClaimSet,
    ) -> Result<Prepared<'c>, Error> {
        authorize(scopes, &set.collection_id)?;
        let admitted = admit(set)?;
        let mut originals = BTreeMap::new();
        for support in admitted.iter().flat_map(|claim| &claim.supports) {
            let original = match originals.entry(support.revision_id.as_str()) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => entry.insert(self.claim_original(
                    scopes,
                    &set.collection_id,
                    &support.revision_id,
                )?),
            };
            original.check_quote(support.span, &support.quote_digest)?;
        }
        Ok(Prepared {
            collection: &set.collection_id,
            admitted,
        })
    }
}

/// Claims whose form and original bytes have been verified outside the write lock.
pub(super) struct Prepared<'c> {
    /// Collection owning every claim.
    collection: &'c str,
    /// Claims in caller order with canonical identities.
    admitted: Vec<Admitted<'c>>,
}

impl Prepared<'_> {
    /// Stable identities in batch order.
    pub(super) fn ids(&self) -> Vec<&str> {
        self.admitted
            .iter()
            .map(|claim| claim.id.as_str())
            .collect()
    }

    /// Recheck mutable eligibility and record claims without creating a set.
    pub(super) fn record_claims(&self, transaction: &Transaction<'_>) -> Result<(), Error> {
        for admitted in &self.admitted {
            for support in &admitted.supports {
                eligible(transaction, &support.revision_id)?;
            }
            let recorded = transaction
                .query_row(
                    "SELECT 1 FROM claims WHERE id = ?1",
                    [admitted.id.as_str()],
                    |_| Ok(()),
                )
                .optional()?;
            if recorded.is_none() {
                insert_claim(transaction, self.collection, admitted)?;
            }
        }
        Ok(())
    }

    /// Record the complete set and hydrate its receipt before committing.
    pub(super) fn record_set(
        &self,
        transaction: &Transaction<'_>,
        scopes: &ScopeSet,
    ) -> Result<ClaimSetRecord, Error> {
        self.record_claims(transaction)?;
        freeze_set(transaction, scopes, self.collection, &self.ids())
    }
}

/// Refuses a collection `scopes` does not cover.
pub(super) fn authorize(scopes: &ScopeSet, collection: &str) -> Result<(), Error> {
    check_collection_name(collection).map_err(|_| Error::Unauthorized)?;
    let scope: Scope = collection_path(collection)
        .parse()
        .map_err(|_| Error::Unauthorized)?;
    if scopes.covers(&scope) {
        Ok(())
    } else {
        Err(Error::Unauthorized)
    }
}

/// The claims of `set` with their ids, once each has the right form and
/// none is there twice.
fn admit(set: &ClaimSet) -> Result<Vec<Admitted<'_>>, Error> {
    if set.claims.is_empty() {
        return Err(Error::Invalid("a claim set holds no claim".to_owned()));
    }
    let mut seen = BTreeSet::new();
    let mut admitted = Vec::with_capacity(set.claims.len());
    for claim in &set.claims {
        let claim = admit_claim(&set.collection_id, claim)?;
        if !seen.insert(claim.id.clone()) {
            return Err(Error::Invalid(format!(
                "the claim {} is in the set twice",
                claim.id.as_str()
            )));
        }
        admitted.push(claim);
    }
    Ok(admitted)
}

/// `claim` with its id and ordered supports, once it has the right form.
fn admit_claim<'c>(collection: &str, claim: &'c Claim) -> Result<Admitted<'c>, Error> {
    let invalid = |reason: &str| Err(Error::Invalid(reason.to_owned()));
    if claim.subject.name.is_empty() {
        return invalid("a claim's subject name is empty");
    }
    check_object(claim)?;
    if claim.provenance.extractor.is_empty() {
        return invalid("a claim's extractor is empty");
    }
    if claim.conditions.contains_key("") {
        return invalid("a claim's condition has an empty name");
    }
    if !(bounded(&claim.version) && bounded(&claim.world)) {
        return invalid("a claim's validity has an empty bound");
    }
    if claim.supports.is_empty() {
        return invalid("a claim has no support");
    }
    let mut supports: Vec<&Support> = claim.supports.iter().collect();
    supports.sort_by(|left, right| location(left).cmp(&location(right)));
    let locations: BTreeSet<_> = supports.iter().map(|support| location(support)).collect();
    if locations.len() < supports.len() {
        return invalid("a claim cites one support twice");
    }
    if supports.iter().any(|support| support.block_id.is_empty()) {
        return invalid("a claim's support names no block");
    }
    if supports
        .iter()
        .any(|support| support.span.start >= support.span.end)
    {
        return invalid("a claim's support has an empty span");
    }
    let id = claim_digest(collection, claim.subject.kind.as_str(), claim, &supports);
    Ok(Admitted {
        id,
        claim,
        supports,
    })
}

/// The G02 canonical identity, retaining the legacy kind spelling and ordered supports.
pub(super) fn claim_digest(
    collection: &str,
    subject_kind: &str,
    claim: &Claim,
    supports: &[&Support],
) -> Digest {
    let (object_kind, object_text) = object_columns(&claim.object);
    canonical_digest(&json!([
        "maestro-claim/1",
        collection,
        subject_kind,
        claim.subject.name,
        claim.predicate.as_str(),
        object_kind,
        object_text,
        claim.conditions.iter().collect::<Vec<_>>(),
        validity(&claim.version),
        validity(&claim.world),
        claim.provenance.extractor,
        claim.provenance.profile.as_str(),
        supports
            .iter()
            .map(|support| json!([
                support.revision_id,
                support.block_id,
                support.span.start,
                support.span.end,
                support.quote_digest.as_str()
            ]))
            .collect::<Vec<_>>()
    ]))
}

/// The G02 canonical identity of an ordered collection of claim ids.
pub(super) fn set_digest(collection: &str, ids: &[impl Serialize]) -> Digest {
    canonical_digest(&json!(["maestro-claim-set/1", collection, ids]))
}

/// Refuses a claim whose predicate is `ALIAS_OF`, or whose object is not the
/// one its predicate takes: a literal of the form of its type for
/// `DEFAULTS_TO`, an entity with a name for every other predicate.
fn check_object(claim: &Claim) -> Result<(), Error> {
    let predicate = claim.predicate.as_str();
    if !claim.predicate.is_claimable() {
        return Err(Error::Invalid(format!(
            "{predicate} is a reviewed identity record, never a claim"
        )));
    }
    match &claim.object {
        Object::Literal(_) if !claim.predicate.takes_literal() => Err(Error::Invalid(format!(
            "the object of {predicate} is an entity, not a literal"
        ))),
        Object::Entity(_) if claim.predicate.takes_literal() => Err(Error::Invalid(format!(
            "the object of {predicate} is a literal, not an entity"
        ))),
        Object::Literal(literal) if !literal.kind.admits(&literal.lexeme) => {
            Err(Error::Invalid(format!(
                "the lexeme {:?} is not a {}",
                literal.lexeme,
                literal.kind.as_str()
            )))
        }
        Object::Entity(entity) if entity.name.is_empty() => {
            Err(Error::Invalid("a claim's object name is empty".to_owned()))
        }
        Object::Literal(_) | Object::Entity(_) => Ok(()),
    }
}

/// The two texts that name `object` in a claim's canonical form: a
/// literal's type and lexeme, as 0012 recorded them, or an entity's kind and
/// name. The predicate before them says which.
fn object_columns(object: &Object) -> (&'static str, &str) {
    match object {
        Object::Literal(literal) => (literal.kind.as_str(), &literal.lexeme),
        Object::Entity(entity) => (entity.kind.as_str(), &entity.name),
    }
}

/// Where a support lies: what identifies it within its claim.
fn location(support: &Support) -> (&str, &str, usize, usize) {
    (
        &support.revision_id,
        &support.block_id,
        support.span.start,
        support.span.end,
    )
}

/// Whether `validity` has no empty bound.
fn bounded(validity: &Validity) -> bool {
    match validity {
        Validity::Unknown => true,
        Validity::Bounded { start, end } => {
            start.as_deref() != Some("") && end.as_deref() != Some("")
        }
    }
}

/// `validity` in a claim's canonical form: null when unknown.
fn validity(validity: &Validity) -> Value {
    match validity {
        Validity::Unknown => Value::Null,
        Validity::Bounded { start, end } => json!([start, end]),
    }
}

/// The SHA-256 of `value`'s compact JSON: arrays only, so its order is its
/// own whatever the JSON library orders objects by.
fn canonical_digest(value: &Value) -> Digest {
    Digest::of(value.to_string().as_bytes())
}

/// Freeze already admitted claims in a canonical, ordered set.
pub(super) fn freeze_set(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    collection: &str,
    ids: &[&str],
) -> Result<ClaimSetRecord, Error> {
    let id = set_digest(collection, ids);
    let recorded = transaction
        .query_row(
            "SELECT 1 FROM claim_sets WHERE id = ?1",
            [id.as_str()],
            |_| Ok(()),
        )
        .optional()?;
    if recorded.is_none() {
        transaction.execute(
            "INSERT INTO claim_sets (id, collection_id, member_count) VALUES (?1, ?2, ?3)",
            params![id.as_str(), collection, integer(ids.len())],
        )?;
        for (ordinal, claim) in ids.iter().enumerate() {
            transaction.execute(
                "INSERT INTO claim_set_members (claim_set_id, ordinal, claim_id)
                 VALUES (?1, ?2, ?3)",
                params![id.as_str(), integer(ordinal), claim],
            )?;
        }
    }
    load_set(transaction, scopes, &id)?.ok_or(Error::Unauthorized)
}

/// Records `admitted` of `collection`, unreviewed, with its supports.
fn insert_claim(
    transaction: &Transaction<'_>,
    collection: &str,
    admitted: &Admitted<'_>,
) -> Result<(), Error> {
    let claim = admitted.claim;
    let (version_known, version_start, version_end) = columns(&claim.version);
    let (world_known, world_start, world_end) = columns(&claim.world);
    let conditions = json!(claim.conditions).to_string();
    let (literal, entity) = match &claim.object {
        Object::Literal(literal) => (Some((literal.kind.as_str(), &literal.lexeme)), None),
        Object::Entity(entity) => (None, Some((entity.kind.as_str(), &entity.name))),
    };
    transaction.execute(
        "INSERT INTO claims (id, collection_id, subject_kind, subject_name, predicate,
           object_type, object_lexeme, object_kind, object_name, conditions_json,
           version_known, version_start, version_end, world_known, world_start, world_end,
           extractor, profile_digest, support_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
           ?18, ?19)",
        params![
            admitted.id.as_str(),
            collection,
            claim.subject.kind.as_str(),
            claim.subject.name,
            claim.predicate.as_str(),
            literal.map(|(kind, _)| kind),
            literal.map(|(_, lexeme)| lexeme),
            entity.map(|(kind, _)| kind),
            entity.map(|(_, name)| name),
            conditions,
            version_known,
            version_start,
            version_end,
            world_known,
            world_start,
            world_end,
            claim.provenance.extractor,
            claim.provenance.profile.as_str(),
            integer(admitted.supports.len()),
        ],
    )?;
    for support in &admitted.supports {
        transaction.execute(
            "INSERT INTO claim_supports (claim_id, revision_id, block_id, span_start, span_end,
               quote_digest)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                admitted.id.as_str(),
                support.revision_id,
                support.block_id,
                integer(support.span.start),
                integer(support.span.end),
                support.quote_digest.as_str(),
            ],
        )?;
    }
    Ok(())
}

/// The `known`, `start` and `end` columns of `validity`.
fn columns(validity: &Validity) -> (bool, Option<&str>, Option<&str>) {
    match validity {
        Validity::Unknown => (false, None, None),
        Validity::Bounded { start, end } => (true, start.as_deref(), end.as_deref()),
    }
}

/// `value` as SQLite stores it. Nothing held in memory counts past
/// `i64::MAX`, and a span past it was refused as out of range already.
fn integer(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
