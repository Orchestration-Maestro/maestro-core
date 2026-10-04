//! Immutable 0031 reservation authority; runtime migration activation is coupled
//! to the complete producer/native cutover.
use super::{
    error::Error, projection_binding::EXACT_RESOLVER_VERSION, projection_inputs::validate_inputs,
    projection_records,
};
use crate::{
    artifact::Digest,
    job::{self, Lease, NewJob},
    scope::{ScopeSet, collection_path},
};
use rusqlite::{OptionalExtension as _, Transaction, params};
use serde_json::{Value, json};
use std::time::SystemTime;
use ulid::Ulid;

/// Exact frozen inputs; runtime builds always use native format /3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Owning collection.
    pub collection_id: String,
    /// Attached generation.
    pub generation_id: i64,
    /// Immutable claim set.
    pub claim_set_id: Digest,
    /// Covering resolution.
    pub resolution_id: Digest,
    /// Supported resolver identity.
    pub resolver_version: String,
    /// Admitted settings identity.
    pub settings_identity: Digest,
    /// Complete frozen lock identity.
    pub frozen_lock: Digest,
    /// Compare-and-swap predecessor, absent only for initial publication.
    pub expected_active_build_id: Option<i64>,
}

impl Request {
    /// One canonical encoding for job submission and lease validation.
    #[must_use]
    pub fn inputs(&self) -> Value {
        json!({"schema": "graph-project/2", "generation": self.generation_id,
            "claim_set": self.claim_set_id.as_str(), "resolution": self.resolution_id.as_str(),
            "resolver": self.resolver_version, "settings": self.settings_identity.as_str(),
            "lock": self.frozen_lock.as_str(), "format": "maestro-typed-edges/3",
            "expected_active_build": self.expected_active_build_id})
    }
}

/// A reserved identity, not a second job state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reservation {
    /// Never-recycled native build identity.
    pub build_id: i64,
    /// Exact project attempt.
    pub job: Ulid,
    /// Persisted inputs.
    pub request: Request,
}

/// Reserve or replay under the current lease, without resetting progress.
pub(super) fn begin(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    request: &Request,
    lease: &Lease,
    now: SystemTime,
) -> Result<Reservation, Error> {
    admit(transaction, scopes, request, lease, now)?;
    if let Some(reserved) = by_job(transaction, scopes, lease.job)? {
        if &reserved.request != request {
            return Err(Error::Conflict("projection build inputs changed".into()));
        }
        return validate(transaction, scopes, reserved.build_id, lease, now);
    }
    transaction.execute(
        "INSERT INTO graph_projection_builds
         (generation_id, collection_id, claim_set_id, project_job_id, expected_active_build_id,
          schema_version, resolution_id, resolver_version, settings_identity, frozen_lock)
         VALUES (?1, ?2, ?3, ?4, ?5, 'maestro-typed-edges/3', ?6, ?7, ?8, ?9)",
        params![
            request.generation_id,
            request.collection_id,
            request.claim_set_id.as_str(),
            lease.job.to_string(),
            request.expected_active_build_id,
            request.resolution_id.as_str(),
            request.resolver_version,
            request.settings_identity.as_str(),
            request.frozen_lock.as_str()
        ],
    )?;
    Ok(Reservation {
        build_id: transaction.last_insert_rowid(),
        job: lease.job,
        request: request.clone(),
    })
}

/// Scoped lookup for explicit recovery; cached records grant no authority.
pub(super) fn by_build(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    build: i64,
) -> Result<Option<Reservation>, Error> {
    lookup(transaction, scopes, Some(build), None)
}

/// Scoped lookup for the exact attempt.
pub(super) fn by_job(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    job: Ulid,
) -> Result<Option<Reservation>, Error> {
    lookup(transaction, scopes, None, Some(job))
}

/// Recheck a reserved build and its live lease before native work.
pub(super) fn validate(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    build: i64,
    lease: &Lease,
    now: SystemTime,
) -> Result<Reservation, Error> {
    let reserved = by_build(transaction, scopes, build)?.ok_or(Error::Unauthorized)?;
    if reserved.job != lease.job {
        return Err(Error::Unauthorized);
    }
    admit(transaction, scopes, &reserved.request, lease, now)?;
    let published: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM graph_projection_receipts WHERE build_id = ?1)",
        [build],
        |row| row.get(0),
    )?;
    if published {
        return Err(Error::Conflict(
            "projection build already has a receipt".into(),
        ));
    }
    Ok(reserved)
}

/// One scoped decoder for new reserved records, not retained legacy receipts.
fn lookup(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    build: Option<i64>,
    job: Option<Ulid>,
) -> Result<Option<Reservation>, Error> {
    let row = transaction
        .query_row(
            &format!(
                "SELECT b.build_id, b.project_job_id, b.collection_id, b.generation_id,
            b.claim_set_id, b.resolution_id, b.resolver_version, b.settings_identity,
            b.frozen_lock, b.expected_active_build_id, b.schema_version
            FROM graph_projection_builds b JOIN generations g ON g.id = b.generation_id
            WHERE ((?1 IS NOT NULL AND b.build_id = ?1) OR
                   (?2 IS NOT NULL AND b.project_job_id = ?2))
            AND b.project_job_id IS NOT NULL AND b.collection_id = g.collection_id AND {}",
                ScopeSet::collection_condition("g.collection_id", 3)
            ),
            params![build, job.map(|id| id.to_string()), scopes.parameter()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, Option<i64>>(9)?,
                    row.get::<_, String>(10)?,
                ))
            },
        )
        .optional()?;
    row.map(
        |(
            build_id,
            job,
            collection_id,
            generation_id,
            set,
            resolution,
            resolver_version,
            settings,
            lock,
            expected_active_build_id,
            schema,
        )| {
            let invalid = || Error::Conflict("invalid reserved projection inputs".into());
            if build_id <= 0
                || generation_id <= 0
                || schema != "maestro-typed-edges/3"
                || resolver_version != EXACT_RESOLVER_VERSION
                || expected_active_build_id.is_some_and(|id| id <= 0)
            {
                return Err(invalid());
            }
            let pin = |value: &str| Digest::parse(value).map_err(|_| invalid());
            Ok(Reservation {
                build_id,
                job: job.parse().map_err(|_| invalid())?,
                request: Request {
                    collection_id,
                    generation_id,
                    claim_set_id: pin(&set)?,
                    resolution_id: pin(&resolution)?,
                    resolver_version,
                    settings_identity: pin(&settings)?,
                    frozen_lock: pin(&lock)?,
                    expected_active_build_id,
                },
            })
        },
    )
    .transpose()
}

/// Current scope, attachment, resolution, lease and eligible predecessor authority.
fn admit(
    transaction: &Transaction<'_>,
    scopes: &ScopeSet,
    request: &Request,
    lease: &Lease,
    now: SystemTime,
) -> Result<(), Error> {
    let scope = collection_path(&request.collection_id)
        .parse()
        .map_err(|_| Error::Unauthorized)?;
    if !scopes.covers(&scope) || request.generation_id <= 0 {
        return Err(Error::Unauthorized);
    }
    let holder = job::validate_lease(transaction, lease)?;
    let timestamp = job::timestamp(transaction, Some(now))?;
    if holder
        .lease
        .as_ref()
        .is_none_or(|current| current.expires <= timestamp)
    {
        return Err(Error::Job(job::Error::Lost {
            job: lease.job,
            holder: lease.holder.clone(),
            number: lease.number,
        }));
    }
    let inputs = request.inputs();
    let expected = NewJob {
        kind: "knowledge.graph.project",
        inputs: &inputs,
        scope: &scope,
        resource: None,
    };
    if holder.scope != scope
        || holder.kind != expected.kind
        || holder.resource.is_some()
        || holder.idempotency_key != job::idempotency_key(&expected)
    {
        return Err(Error::Unauthorized);
    }
    validate_inputs(
        transaction,
        scopes,
        &request.claim_set_id,
        &request.resolution_id,
        &request.resolver_version,
    )?;
    if let Some(previous) = request.expected_active_build_id {
        projection_records::by_build(transaction, scopes, request.generation_id, previous)?
            .ok_or(Error::Unauthorized)?;
    }
    validate_head(transaction, request)
}

/// NULL-safe initial/replacement state and pinned predecessor checks.
fn validate_head(transaction: &Transaction<'_>, request: &Request) -> Result<(), Error> {
    let eligible: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM generations g
         JOIN graph_attachments a ON a.generation_id = g.id
         LEFT JOIN graph_projection_active h ON h.generation_id = g.id
         LEFT JOIN graph_projection_builds p ON p.build_id = h.build_id AND p.generation_id = g.id
         LEFT JOIN graph_projection_receipts r ON r.build_id = p.build_id
         WHERE g.id = ?1 AND g.collection_id = ?2 AND a.claim_set_id = ?3 AND
         ((?4 IS NULL AND h.build_id IS NULL AND g.state = 'verified') OR
          (?4 IS NOT NULL AND h.build_id = ?4 AND r.build_id IS NOT NULL
           AND g.state IN ('verified', 'published') AND p.claim_set_id = ?3
           AND (p.schema_version = 'maestro-typed-edges/1' OR
                (p.schema_version IN ('maestro-typed-edges/2', 'maestro-typed-edges/3')
                 AND p.resolution_id = ?5 AND p.resolver_version = ?6)))))",
        params![
            request.generation_id,
            request.collection_id,
            request.claim_set_id.as_str(),
            request.expected_active_build_id,
            request.resolution_id.as_str(),
            request.resolver_version
        ],
        |row| row.get(0),
    )?;
    if !eligible {
        return Err(Error::Unauthorized);
    }
    Ok(())
}
