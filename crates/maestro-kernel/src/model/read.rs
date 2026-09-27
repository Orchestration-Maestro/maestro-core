//! Scoped model registry readers with artifact and row-metadata verification.

use super::{
    error::Error,
    records::{
        CARD_COLUMNS, CardRecord, EVALUATION_COLUMNS, EvaluationDisposition, EvaluationMode,
        EvaluationRecord, SELECTION_COLUMNS, SelectedModelCard, card_row, evaluation_row,
        selection_row,
    },
};
use crate::{
    artifact::Digest,
    gateway::{ModelCard, Role},
    scope::ScopeSet,
    store::Database,
};
use rusqlite::{Connection, OptionalExtension as _, params};
use std::str;
use ulid::Ulid;

impl Database {
    /// Reads a card only if its collection is covered by `scopes`; a hidden
    /// or missing digest is `None`, never a distinguishing error.
    ///
    /// # Errors
    ///
    /// Returns an error if the database, artifact store, or integrity checks fail.
    pub fn model_card(
        &self,
        scopes: &ScopeSet,
        collection: &str,
        digest: &Digest,
    ) -> Result<Option<ModelCard>, Error> {
        let reader = self.reader()?;
        card_by_digest(&reader, self, scopes, collection, digest)
            .map(|record| record.map(|(_, card)| card))
    }

    /// Lists one role's scoped card metadata in committed row order.
    ///
    /// # Errors
    ///
    /// Returns an error if the database, artifact store, or integrity checks fail.
    pub fn model_cards(
        &self,
        scopes: &ScopeSet,
        collection: &str,
        role: Role,
    ) -> Result<Vec<CardRecord>, Error> {
        let reader = self.reader()?;
        let mut statement = reader.prepare(&format!(
            "SELECT {CARD_COLUMNS} FROM model_cards \
            WHERE collection_id = ?1 AND role = ?2 AND {} ORDER BY rowid",
            ScopeSet::collection_condition("model_cards.collection_id", 3),
        ))?;
        let rows = statement
            .query_map(
                params![collection, role.to_string(), scopes.parameter()],
                card_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        rows.into_iter()
            .map(|(record, card_json)| {
                load_card(self, &record, &card_json)?;
                Ok(record)
            })
            .collect()
    }

    /// Lists immutable evaluation metadata in committed row order. Both
    /// opaque evidence artifacts are loaded and digest-verified before return.
    ///
    /// # Errors
    ///
    /// Returns an error if the database, evidence artifacts, or integrity checks fail.
    pub fn model_evaluations(
        &self,
        scopes: &ScopeSet,
        collection: &str,
    ) -> Result<Vec<EvaluationRecord>, Error> {
        let reader = self.reader()?;
        let mut statement = reader.prepare(&format!(
            "SELECT {EVALUATION_COLUMNS} FROM model_evaluations \
            WHERE collection_id = ?1 AND {} ORDER BY rowid",
            ScopeSet::collection_condition("model_evaluations.collection_id", 2),
        ))?;
        let records = statement
            .query_map(params![collection, scopes.parameter()], evaluation_row)?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        for record in &records {
            self.get(&record.manifest_digest)?;
            self.get(&record.report_digest)?;
            verify_evaluation_card(&reader, self, scopes, record)?;
        }
        Ok(records)
    }

    /// The latest committed role selection and its exact card/evaluation, or
    /// `None` when the caller lacks scope or no selection was recorded.
    ///
    /// # Errors
    ///
    /// Returns an error if the database, artifacts, or selected metadata fail verification.
    pub fn selected_model_card(
        &self,
        scopes: &ScopeSet,
        collection: &str,
        role: Role,
    ) -> Result<Option<SelectedModelCard>, Error> {
        let reader = self.reader()?;
        let selection = reader
            .query_row(
                &format!(
                    "SELECT {SELECTION_COLUMNS} FROM model_selections \
                    WHERE collection_id = ?1 AND role = ?2 AND {} \
                    ORDER BY rowid DESC LIMIT 1",
                    ScopeSet::collection_condition("model_selections.collection_id", 3),
                ),
                params![collection, role.to_string(), scopes.parameter()],
                selection_row,
            )
            .optional()?;
        let Some(selection) = selection else {
            return Ok(None);
        };
        let evaluation = reader
            .query_row(
                &format!(
                    "SELECT {EVALUATION_COLUMNS} FROM model_evaluations \
                    WHERE id = ?1 AND collection_id = ?2 AND {}",
                    ScopeSet::collection_condition("model_evaluations.collection_id", 3),
                ),
                params![
                    selection.evaluation_id.to_string(),
                    collection,
                    scopes.parameter()
                ],
                evaluation_row,
            )
            .optional()?
            .ok_or_else(|| {
                Error::Integrity("selection evaluation is missing from its collection".to_owned())
            })?;
        self.get(&evaluation.manifest_digest)?;
        self.get(&evaluation.report_digest)?;
        verify_evaluation_card(&reader, self, scopes, &evaluation)?;
        if evaluation.card_id != selection.card_id
            || evaluation.role != selection.role
            || evaluation.mode != EvaluationMode::Real
            || evaluation.disposition != EvaluationDisposition::Eligible
        {
            return Err(Error::Integrity(
                "selection no longer matches an eligible real evaluation".to_owned(),
            ));
        }
        let (_, card) = card_by_id(self, &reader, scopes, collection, selection.card_id)?
            .ok_or_else(|| {
                Error::Integrity("selection card is missing from its collection".to_owned())
            })?;
        Ok(Some(SelectedModelCard {
            card,
            selection,
            evaluation,
        }))
    }
}

/// Loads and verifies one registered card against its database JSON.
fn load_card(
    database: &Database,
    record: &CardRecord,
    card_json: &str,
) -> Result<ModelCard, Error> {
    let bytes = database.get(&record.digest)?;
    let artifact_text =
        str::from_utf8(&bytes).map_err(|error| Error::Integrity(error.to_string()))?;
    if artifact_text != card_json {
        return Err(Error::Integrity(
            "card_json does not equal its digest-verified artifact".to_owned(),
        ));
    }
    let card = ModelCard::from_json_bytes(&bytes)?;
    if card.identity().is_none() || card.fields().role != record.role {
        return Err(Error::Integrity(
            "card artifact metadata does not match its registration".to_owned(),
        ));
    }
    Ok(card)
}

/// Verifies that the evaluation names a registered card of the same role.
fn verify_evaluation_card(
    reader: &Connection,
    database: &Database,
    scopes: &ScopeSet,
    evaluation: &EvaluationRecord,
) -> Result<(), Error> {
    let (record, _) = card_by_id(
        database,
        reader,
        scopes,
        &evaluation.collection_id,
        evaluation.card_id,
    )?
    .ok_or_else(|| Error::Integrity("evaluation card registration is missing".to_owned()))?;
    if record.role != evaluation.role {
        return Err(Error::Integrity(
            "evaluation and card roles differ".to_owned(),
        ));
    }
    Ok(())
}

/// Reads one registration by id without exposing records outside its scope.
pub(super) fn card_by_id(
    database: &Database,
    reader: &Connection,
    scopes: &ScopeSet,
    collection: &str,
    id: Ulid,
) -> Result<Option<(CardRecord, ModelCard)>, Error> {
    let row = reader
        .query_row(
            &format!(
                "SELECT {CARD_COLUMNS} FROM model_cards \
                WHERE id = ?1 AND collection_id = ?2 AND {}",
                ScopeSet::collection_condition("model_cards.collection_id", 3),
            ),
            params![id.to_string(), collection, scopes.parameter()],
            card_row,
        )
        .optional()?;
    row.map(|(record, json)| {
        let card = load_card(database, &record, &json)?;
        Ok((record, card))
    })
    .transpose()
}

/// Loads one card by collection and digest, validating its persisted artifact.
pub(super) fn card_by_digest(
    reader: &Connection,
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
    digest: &Digest,
) -> Result<Option<(CardRecord, ModelCard)>, Error> {
    let row = reader
        .query_row(
            &format!(
                "SELECT {CARD_COLUMNS} FROM model_cards \
                WHERE collection_id = ?1 AND digest = ?2 AND {}",
                ScopeSet::collection_condition("model_cards.collection_id", 3),
            ),
            params![collection, digest.as_str(), scopes.parameter()],
            card_row,
        )
        .optional()?;
    row.map(|(record, json)| {
        let card = load_card(database, &record, &json)?;
        Ok((record, card))
    })
    .transpose()
}

/// Loads one evaluation by collection and ID after checking its evidence and card.
pub(super) fn evaluation_by_id(
    database: &Database,
    scopes: &ScopeSet,
    collection: &str,
    id: Ulid,
) -> Result<Option<EvaluationRecord>, Error> {
    let reader = database.reader()?;
    let evaluation = reader
        .query_row(
            &format!(
                "SELECT {EVALUATION_COLUMNS} FROM model_evaluations \
                WHERE id = ?1 AND collection_id = ?2 AND {}",
                ScopeSet::collection_condition("model_evaluations.collection_id", 3),
            ),
            params![id.to_string(), collection, scopes.parameter()],
            evaluation_row,
        )
        .optional()?;
    if let Some(record) = &evaluation {
        database.get(&record.manifest_digest)?;
        database.get(&record.report_digest)?;
        verify_evaluation_card(&reader, database, scopes, record)?;
    }
    Ok(evaluation)
}
