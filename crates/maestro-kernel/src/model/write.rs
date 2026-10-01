//! Scoped transactional model registry writers.

use super::{
    error::Error,
    read::{card_by_digest, card_by_id, evaluation_by_id},
    records::{
        CARD_COLUMNS, CardRecord, EVALUATION_COLUMNS, EvaluationDisposition, EvaluationMode,
        EvaluationRecord, NewModelCard, NewModelEvaluation, NewModelSelection, SELECTION_COLUMNS,
        SelectionRecord, card_row, evaluation_row, selection_row,
    },
};
use crate::{
    artifact::Digest,
    gateway::{ModelCard, Role},
    journal::{self, NewEvent, event},
    scope::{Scope, ScopeSet, collection_path},
    store::{Database, artifacts},
};
use rusqlite::{OptionalExtension as _, params};
use serde_json::json;
use std::str;
use ulid::Ulid;

/// Journal event emitted when a card is registered.
const CARD_RECORDED: &str = "maestro.model.card.recorded.v1";
/// Journal event emitted when a candidate evaluation is recorded.
const EVALUATION_RECORDED: &str = "maestro.model.evaluation.recorded.v1";
/// Journal event emitted when a role selection is recorded.
const SELECTED: &str = "maestro.model.selected.v1";
/// Artifact media type for card, manifest, and report JSON.
const JSON_MEDIA: &str = "application/json";

impl Database {
    /// Registers a v2 card in the scoped collection registry. An identical
    /// registration is read and returned without another pin or journal row.
    ///
    /// # Errors
    ///
    /// Returns an error for an unauthorized scope, invalid card, failed pin/journal write,
    /// or database/artifact integrity error.
    pub fn record_model_card(
        &self,
        scopes: &ScopeSet,
        new: &NewModelCard<'_>,
    ) -> Result<CardRecord, Error> {
        authorize(scopes, new.collection_id)?;
        ensure_collection(self, new.collection_id)?;
        let identity = new.card.identity().ok_or_else(|| {
            Error::Invalid("a v1 card cannot be registered as a candidate".to_owned())
        })?;
        let json_bytes = new.card.card_json()?;
        let json_text =
            str::from_utf8(&json_bytes).map_err(|error| Error::Integrity(error.to_string()))?;

        let reader = self.reader()?;
        if let Some((existing, _)) =
            card_by_digest(&reader, self, scopes, new.collection_id, new.card.digest())?
        {
            return Ok(existing);
        }
        let digest = self.put(&json_bytes, JSON_MEDIA)?;
        let mut pins = identity.artifact_digests();
        pins.insert(digest.clone());
        let role = new.card.fields().role.to_string();
        let record = self.write(|transaction| {
            let existing = transaction
                .query_row(
                    &format!(
                        "SELECT {CARD_COLUMNS} FROM model_cards \
                        WHERE collection_id = ?1 AND digest = ?2"
                    ),
                    params![new.collection_id, digest.as_str()],
                    card_row,
                )
                .optional()?;
            if let Some((record, stored_json)) = existing {
                return match (
                    stored_json == json_text,
                    record.role == new.card.fields().role,
                ) {
                    (true, true) => Ok(record),
                    _ => Err(Error::Integrity(
                        "existing card row differs from its v2 artifact".to_owned(),
                    )),
                };
            }
            let (inserted, _) = transaction.query_row(
                &format!(
                    "INSERT INTO model_cards (id, collection_id, role, digest, card_json) \
                    VALUES (?1, ?2, ?3, ?4, ?5) RETURNING {CARD_COLUMNS}"
                ),
                params![
                    Ulid::generate().to_string(),
                    new.collection_id,
                    role,
                    digest.as_str(),
                    json_text
                ],
                card_row,
            )?;
            for artifact in &pins {
                artifacts::pin(transaction, artifact)?;
            }
            journal_card(transaction, &inserted)?;
            Ok(inserted)
        })?;
        Ok(record)
    }

    /// Records an immutable evaluation and its opaque manifest/report artifacts.
    /// A preflight failure may have no generation; distinct calls remain distinct.
    ///
    /// # Errors
    ///
    /// Returns an error for an unauthorized or inconsistent record, a failed artifact pin or
    /// journal write, or a database/artifact integrity error.
    pub fn record_model_evaluation(
        &self,
        scopes: &ScopeSet,
        new: &NewModelEvaluation<'_>,
    ) -> Result<EvaluationRecord, Error> {
        authorize(scopes, new.collection_id)?;
        ensure_collection(self, new.collection_id)?;
        nonblank("run_id", new.run_id)?;
        let (card, _) = registered_card(self, scopes, new.collection_id, new.card_id, new.role)?;
        // Refuse before storing evidence; the INSERT trigger rechecks ownership atomically.
        validate_generation(self, new.collection_id, new.generation_id)?;
        let manifest_digest = self.put(new.manifest, JSON_MEDIA)?;
        let report_digest = self.put(new.report, JSON_MEDIA)?;
        self.write(|transaction| {
            let record = transaction.query_row(
                &format!(
                    "INSERT INTO model_evaluations \
                    (id, run_id, collection_id, card_id, role, mode, generation_id, \
                    disposition, manifest_digest, report_digest) \
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
                    RETURNING {EVALUATION_COLUMNS}"
                ),
                params![
                    Ulid::generate().to_string(),
                    new.run_id,
                    new.collection_id,
                    new.card_id.to_string(),
                    new.role.to_string(),
                    mode_text(new.mode),
                    new.generation_id,
                    disposition_text(new.disposition),
                    manifest_digest.as_str(),
                    report_digest.as_str()
                ],
                evaluation_row,
            )?;
            artifacts::pin(transaction, &manifest_digest)?;
            artifacts::pin(transaction, &report_digest)?;
            journal_evaluation(transaction, &record, &card.digest)?;
            Ok(record)
        })
    }

    /// Records a role selection only for the exact registered v2 card and a
    /// real, eligible evaluation of that same card. No selection is implicit.
    ///
    /// # Errors
    ///
    /// Returns an error for an unauthorized or ineligible selection, a failed journal write,
    /// or a database/artifact integrity error.
    pub fn record_model_selection(
        &self,
        scopes: &ScopeSet,
        new: &NewModelSelection<'_>,
    ) -> Result<SelectionRecord, Error> {
        authorize(scopes, new.collection_id)?;
        ensure_collection(self, new.collection_id)?;
        nonblank("selected_by", new.selected_by)?;
        nonblank("reason", new.reason)?;
        let (card_record, card) =
            registered_card(self, scopes, new.collection_id, new.card_id, new.role)?;
        let evaluation = evaluation_by_id(self, scopes, new.collection_id, new.evaluation_id)?
            .ok_or_else(|| {
                Error::Invalid(
                    "selection evaluation is not registered in this collection".to_owned(),
                )
            })?;
        if evaluation.card_id != card_record.id
            || evaluation.mode != EvaluationMode::Real
            || evaluation.disposition != EvaluationDisposition::Eligible
        {
            return Err(Error::Invalid(
                "selection requires an eligible real evaluation of the exact card and role"
                    .to_owned(),
            ));
        }
        self.write(|transaction| {
            let record = transaction.query_row(
                &format!(
                    "INSERT INTO model_selections \
                    (id, collection_id, role, card_id, evaluation_id, selected_by, reason) \
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                    RETURNING {SELECTION_COLUMNS}"
                ),
                params![
                    Ulid::generate().to_string(),
                    new.collection_id,
                    new.role.to_string(),
                    new.card_id.to_string(),
                    new.evaluation_id.to_string(),
                    new.selected_by,
                    new.reason
                ],
                selection_row,
            )?;
            journal_selection(transaction, &record, card.digest())?;
            Ok(record)
        })
    }
}

/// Refuses a write before its collection can have side effects.
fn authorize(scopes: &ScopeSet, collection: &str) -> Result<(), Error> {
    let scope: Scope = collection_path(collection)
        .parse::<Scope>()
        .map_err(|error| Error::Invalid(error.to_string()))?;
    if scopes.covers(&scope) {
        Ok(())
    } else {
        Err(Error::Unauthorized)
    }
}

/// Requires the collection to exist before adding registry rows.
fn ensure_collection(database: &Database, collection: &str) -> Result<(), Error> {
    let exists = database
        .reader()?
        .query_row(
            "SELECT 1 FROM collections WHERE id = ?1",
            [collection],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "collection {collection:?} is not registered"
        )))
    }
}

/// Resolves a scoped card registration and verifies its artifact.
fn registered_card(
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
    id: Ulid,
    role: Role,
) -> Result<(CardRecord, ModelCard), Error> {
    let reader = database.reader()?;
    card_by_id(database, &reader, scopes, collection, id)?
        .filter(|(record, _)| record.role == role)
        .ok_or_else(|| {
            Error::Invalid(
                "card registration is not present in this collection and role".to_owned(),
            )
        })
}

/// Checks a generation belongs to the requested collection.
fn validate_generation(
    database: &Database,
    collection: &str,
    generation: Option<i64>,
) -> Result<(), Error> {
    if let Some(id) = generation {
        let exists = database
            .reader()?
            .query_row(
                "SELECT 1 FROM generations WHERE id = ?1 AND collection_id = ?2",
                params![id, collection],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            return Err(Error::Invalid(
                "generation does not belong to the evaluation collection".to_owned(),
            ));
        }
    }
    Ok(())
}

/// Requires a nonblank caller-supplied identity or explanation.
fn nonblank(field: &str, value: &str) -> Result<(), Error> {
    if value.trim().is_empty() {
        Err(Error::Invalid(format!("{field} is blank")))
    } else {
        Ok(())
    }
}

/// Maps an evaluation mode to its constrained database value.
fn mode_text(mode: EvaluationMode) -> &'static str {
    match mode {
        EvaluationMode::Real => "real",
        EvaluationMode::Synthetic => "synthetic",
    }
}
/// Maps an evaluation disposition to its constrained database value.
fn disposition_text(disposition: EvaluationDisposition) -> &'static str {
    match disposition {
        EvaluationDisposition::Eligible => "eligible",
        EvaluationDisposition::Ineligible => "ineligible",
        EvaluationDisposition::Failed => "failed",
        EvaluationDisposition::Blocked => "blocked",
        EvaluationDisposition::Interrupted => "interrupted",
    }
}

/// Records the card registration event in the same transaction.
fn journal_card(transaction: &rusqlite::Transaction<'_>, card: &CardRecord) -> Result<(), Error> {
    let digest = card.digest.as_str();
    let data = json!({
        "card": card.id.to_string(),
        "collection": card.collection_id,
        "role": card.role.to_string(),
        "card_digest": digest
    });
    journal(
        transaction,
        CARD_RECORDED,
        &format!("model-card/{}", card.id),
        &card.collection_id,
        &data,
    )
}

/// Records evaluation metadata and its exact card digest in the journal.
fn journal_evaluation(
    transaction: &rusqlite::Transaction<'_>,
    evaluation: &EvaluationRecord,
    card_digest: &Digest,
) -> Result<(), Error> {
    let data = json!({
        "evaluation": evaluation.id.to_string(),
        "collection": evaluation.collection_id,
        "card": evaluation.card_id.to_string(),
        "card_digest": card_digest.as_str(),
        "role": evaluation.role.to_string(),
        "mode": mode_text(evaluation.mode),
        "disposition": disposition_text(evaluation.disposition),
        "manifest_digest": evaluation.manifest_digest.as_str(),
        "report_digest": evaluation.report_digest.as_str()
    });
    journal(
        transaction,
        EVALUATION_RECORDED,
        &format!("model-evaluation/{}", evaluation.id),
        &evaluation.collection_id,
        &data,
    )
}

/// Records selection metadata and its exact card digest in the journal.
fn journal_selection(
    transaction: &rusqlite::Transaction<'_>,
    selection: &SelectionRecord,
    card_digest: &Digest,
) -> Result<(), Error> {
    let data = json!({
        "selection": selection.id.to_string(),
        "collection": selection.collection_id,
        "card": selection.card_id.to_string(),
        "card_digest": card_digest.as_str(),
        "evaluation": selection.evaluation_id.to_string(),
        "role": selection.role.to_string(),
        "selected_by": selection.selected_by,
        "reason": selection.reason
    });
    journal(
        transaction,
        SELECTED,
        &format!("model-selection/{}", selection.id),
        &selection.collection_id,
        &data,
    )
}

/// Writes an internal event scoped to the owning collection.
fn journal(
    transaction: &rusqlite::Transaction<'_>,
    event_type: &str,
    subject: &str,
    collection: &str,
    data: &serde_json::Value,
) -> Result<(), Error> {
    event::record(
        transaction,
        &NewEvent {
            stream: &journal::stream(collection),
            r#type: event_type,
            subject,
            scope: &collection_path(collection),
            data,
        },
    )?;
    Ok(())
}
