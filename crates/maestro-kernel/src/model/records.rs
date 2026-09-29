//! Public metadata for immutable model registrations, evaluations and selections.

use crate::{
    artifact::Digest,
    gateway::{ModelCard, Role},
};
use rusqlite::{Row, types::Type};
use std::{error, io};
use ulid::Ulid;

/// Columns selected when reading a registered card.
pub(super) const CARD_COLUMNS: &str = "id, collection_id, role, digest, card_json, recorded_at";
/// Columns selected when reading a candidate evaluation.
pub(super) const EVALUATION_COLUMNS: &str = concat!(
    "id, run_id, collection_id, card_id, role, mode, generation_id, ",
    "disposition, manifest_digest, report_digest, recorded_at",
);
/// Columns selected when reading an explicit role selection.
pub(super) const SELECTION_COLUMNS: &str =
    "id, collection_id, role, card_id, evaluation_id, selected_by, reason, recorded_at";

/// A v2 card and the collection that owns this registration.
#[derive(Debug, Clone, Copy)]
pub struct NewModelCard<'a> {
    /// Collection whose scoped registry records the card.
    pub collection_id: &'a str,
    /// Validated v2 card; v1 cards cannot be newly registered.
    pub card: &'a ModelCard,
}

/// A candidate evaluation, including preflight failures without a generation.
#[derive(Debug, Clone, Copy)]
pub struct NewModelEvaluation<'a> {
    /// Immutable caller-assigned run identity.
    pub run_id: &'a str,
    /// Candidate collection.
    pub collection_id: &'a str,
    /// Registration of the exact candidate card.
    pub card_id: Ulid,
    /// Candidate role; must equal its registered card role.
    pub role: Role,
    /// Synthetic mechanics run or a real approved evaluation.
    pub mode: EvaluationMode,
    /// Generation measured, when preparation produced one.
    pub generation_id: Option<i64>,
    /// Final disposition. See the module contract for its hard-gate meaning.
    pub disposition: EvaluationDisposition,
    /// Opaque manifest bytes, stored and pinned by digest.
    pub manifest: &'a [u8],
    /// Opaque report bytes, stored and pinned by digest.
    pub report: &'a [u8],
}

/// Whether an evaluation is synthetic mechanics or a real trial.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationMode {
    /// Public CI or fake-model run; never selectable.
    Synthetic,
    /// Real run whose authorizations and hard-gate proof are owned by T030b.
    Real,
}

/// Final evaluation disposition and its hard-constraint semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationDisposition {
    /// Every required hard constraint was measured and passed.
    Eligible,
    /// A measured hard constraint failed.
    Ineligible,
    /// A required hard-constraint measurement is missing.
    Blocked,
    /// The run or its report failed.
    Failed,
    /// The run was interrupted before completion.
    Interrupted,
}

/// An explicit, append-only role selection.
#[derive(Debug, Clone, Copy)]
pub struct NewModelSelection<'a> {
    /// Collection whose role is selected.
    pub collection_id: &'a str,
    /// Role selected from the registered candidate.
    pub role: Role,
    /// Exact card registration, not a router alias.
    pub card_id: Ulid,
    /// Eligible real evaluation of that exact card.
    pub evaluation_id: Ulid,
    /// Human or process recording the decision.
    pub selected_by: &'a str,
    /// Nonblank reason retained with the selection history.
    pub reason: &'a str,
}

/// A registered v2 card's scoped metadata, without corpus content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardRecord {
    /// Registration ULID.
    pub id: Ulid,
    /// Owning collection.
    pub collection_id: String,
    /// Registered role.
    pub role: Role,
    /// Exact v2 artifact digest.
    pub digest: Digest,
    /// Database recording time.
    pub recorded_at: String,
}

/// Immutable metadata and evidence digests for one candidate evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationRecord {
    /// Evaluation ULID.
    pub id: Ulid,
    /// Caller-assigned run identity.
    pub run_id: String,
    /// Owning collection.
    pub collection_id: String,
    /// Exact registered card ID.
    pub card_id: Ulid,
    /// Candidate role.
    pub role: Role,
    /// Synthetic or real evaluation mode.
    pub mode: EvaluationMode,
    /// Existing generation, if preparation produced one.
    pub generation_id: Option<i64>,
    /// Final outcome, including blocked and failed attempts.
    pub disposition: EvaluationDisposition,
    /// Pinned opaque manifest artifact digest.
    pub manifest_digest: Digest,
    /// Pinned opaque report artifact digest.
    pub report_digest: Digest,
    /// Database recording time.
    pub recorded_at: String,
}

/// Immutable record that a role was explicitly selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRecord {
    /// Selection ULID.
    pub id: Ulid,
    /// Owning collection.
    pub collection_id: String,
    /// Selected role.
    pub role: Role,
    /// Exact card registration.
    pub card_id: Ulid,
    /// Exact eligible real evaluation.
    pub evaluation_id: Ulid,
    /// Human or process recording the decision.
    pub selected_by: String,
    /// Reason retained with this decision.
    pub reason: String,
    /// Database recording time.
    pub recorded_at: String,
}

/// The selected card joined to the exact selection and evaluation evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedModelCard {
    /// Immutable card identity.
    pub card: ModelCard,
    /// Latest committed selection for this collection and role.
    pub selection: SelectionRecord,
    /// Exact real eligible evaluation named by the selection.
    pub evaluation: EvaluationRecord,
}

/// Parses card metadata and its strict JSON column from one SQLite row.
pub(super) fn card_row(row: &Row<'_>) -> rusqlite::Result<(CardRecord, String)> {
    let id: String = row.get(0)?;
    let role: String = row.get(2)?;
    let digest: String = row.get(3)?;
    Ok((
        CardRecord {
            id: ulid(&id, 0)?,
            collection_id: row.get(1)?,
            role: parse_role(&role, 2)?,
            digest: parse_digest(&digest, 3)?,
            recorded_at: row.get(5)?,
        },
        row.get(4)?,
    ))
}

/// Parses an immutable evaluation row into typed metadata.
pub(super) fn evaluation_row(row: &Row<'_>) -> rusqlite::Result<EvaluationRecord> {
    let id: String = row.get(0)?;
    let card_id: String = row.get(3)?;
    let role: String = row.get(4)?;
    let mode: String = row.get(5)?;
    let disposition: String = row.get(7)?;
    let manifest: String = row.get(8)?;
    let report: String = row.get(9)?;
    Ok(EvaluationRecord {
        id: ulid(&id, 0)?,
        run_id: row.get(1)?,
        collection_id: row.get(2)?,
        card_id: ulid(&card_id, 3)?,
        role: parse_role(&role, 4)?,
        mode: parse_mode(&mode, 5)?,
        generation_id: row.get(6)?,
        disposition: parse_disposition(&disposition, 7)?,
        manifest_digest: parse_digest(&manifest, 8)?,
        report_digest: parse_digest(&report, 9)?,
        recorded_at: row.get(10)?,
    })
}

/// Parses an immutable selection row into typed metadata.
pub(super) fn selection_row(row: &Row<'_>) -> rusqlite::Result<SelectionRecord> {
    let id: String = row.get(0)?;
    let role: String = row.get(2)?;
    let card_id: String = row.get(3)?;
    let evaluation_id: String = row.get(4)?;
    Ok(SelectionRecord {
        id: ulid(&id, 0)?,
        collection_id: row.get(1)?,
        role: parse_role(&role, 2)?,
        card_id: ulid(&card_id, 3)?,
        evaluation_id: ulid(&evaluation_id, 4)?,
        selected_by: row.get(5)?,
        reason: row.get(6)?,
        recorded_at: row.get(7)?,
    })
}

/// Parses a digest column or reports a typed SQLite conversion error.
pub(super) fn parse_digest(text: &str, index: usize) -> rusqlite::Result<Digest> {
    Digest::parse(text).map_err(|error| unreadable(index, error))
}

/// Parses a ULID column or reports a typed SQLite conversion error.
pub(super) fn ulid(text: &str, index: usize) -> rusqlite::Result<Ulid> {
    Ulid::from_string(text).map_err(|error| unreadable(index, error))
}

/// Parses one of the recorded model roles.
pub(super) fn parse_role(text: &str, index: usize) -> rusqlite::Result<Role> {
    enum_value(text, index, "model role", |value| match value {
        "embedder" => Some(Role::Embedder),
        "reranker" => Some(Role::Reranker),
        "answerer" => Some(Role::Answerer),
        "extractor" => Some(Role::Extractor),
        _ => None,
    })
}

/// Parses the synthetic or real evaluation mode.
pub(super) fn parse_mode(text: &str, index: usize) -> rusqlite::Result<EvaluationMode> {
    enum_value(text, index, "evaluation mode", |value| match value {
        "real" => Some(EvaluationMode::Real),
        "synthetic" => Some(EvaluationMode::Synthetic),
        _ => None,
    })
}

/// Parses one immutable evaluation disposition.
pub(super) fn parse_disposition(
    text: &str,
    index: usize,
) -> rusqlite::Result<EvaluationDisposition> {
    enum_value(text, index, "evaluation disposition", |value| match value {
        "eligible" => Some(EvaluationDisposition::Eligible),
        "ineligible" => Some(EvaluationDisposition::Ineligible),
        "failed" => Some(EvaluationDisposition::Failed),
        "blocked" => Some(EvaluationDisposition::Blocked),
        "interrupted" => Some(EvaluationDisposition::Interrupted),
        _ => None,
    })
}

/// Parses a named text enum while preserving SQLite column-error context.
fn enum_value<T>(
    text: &str,
    index: usize,
    kind: &str,
    parse: impl FnOnce(&str) -> Option<T>,
) -> rusqlite::Result<T> {
    parse(text)
        .ok_or_else(|| unreadable(index, io::Error::other(format!("unknown {kind} {text:?}"))))
}

/// Builds a SQLite conversion error with the source cause retained.
pub(super) fn unreadable(
    index: usize,
    source: impl error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(source))
}
