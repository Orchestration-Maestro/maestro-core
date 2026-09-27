//! The kernel-owned search derivatives of one published generation.

use super::{Error, Unverified, point::point_id, projection::Projection};
use crate::{
    prepare::search_members,
    query::{PROFILE, index_identifiers},
};
use maestro_kernel::{
    chunk_set::{Chunk, ChunkSet},
    gateway::ModelPort,
    generation::{Generation, GenerationState},
    retrieval::{Error as RetrievalError, SearchInput, SearchMember},
    scope::ScopeSet,
    store::Database,
};
use qdrant_client::qdrant::{
    PayloadSchemaType, RetrievedPoint, point_id::PointIdOptions, value::Kind,
};
use std::collections::HashMap;

/// Starts or resumes the generation's exact identifier profile.
pub(super) fn begin(
    database: &Database,
    scopes: &ScopeSet,
    generation: &Generation,
) -> Result<bool, Error> {
    database
        .begin_generation_search(scopes, generation.id, PROFILE)
        .map_err(Error::Search)
}

/// Records the manifest's complete successful document membership.
pub(super) fn record_members(
    database: &Database,
    scopes: &ScopeSet,
    set: &ChunkSet,
) -> Result<Vec<SearchMember>, Error> {
    let members = search_members(database, scopes, set).map_err(Error::Preparation)?;
    database
        .record_search_members(scopes, &set.id, &members)
        .map_err(Error::Search)?;
    Ok(members)
}

/// Writes each prepared input and its extracted identifiers before its point
/// is upserted and its progress acknowledged.
pub(super) fn record_batch(
    database: &Database,
    scopes: &ScopeSet,
    chunk_set_id: &str,
    chunks: &[Chunk],
    prepared_inputs: &[String],
) -> Result<(), Error> {
    if chunks.len() != prepared_inputs.len() {
        return Err(Error::Search(RetrievalError::InvalidInput(
            "a search-input batch has mismatched chunks and prepared inputs".to_owned(),
        )));
    }
    let inputs: Vec<SearchInput> = chunks
        .iter()
        .zip(prepared_inputs)
        .map(|(chunk, prepared_input)| SearchInput {
            chunk_id: chunk.id.clone(),
            prepared_input: prepared_input.clone(),
            identifiers: index_identifiers(prepared_input),
        })
        .collect();
    database
        .record_search_inputs(scopes, chunk_set_id, &inputs)
        .map_err(Error::Search)
}

impl<P: ModelPort> Projection<'_, P> {
    /// Rechecks manifest membership, every prepared input, each exact payload
    /// and its keyword indexes before making this generation searchable.
    pub(super) async fn verify_search(
        &self,
        generation: &Generation,
        set: &ChunkSet,
        chunks: &[Chunk],
    ) -> Result<Result<(), Unverified>, Error> {
        if let Err(error) = record_members(self.database, self.scopes, set) {
            return match verification_write_error(error) {
                Ok(reason) => Ok(Err(reason)),
                Err(error) => Err(error),
            };
        }
        let indexes = self
            .qdrant
            .payload_indexes(&super::collection_name(generation))
            .await
            .map_err(Error::Qdrant)?;
        for field in ["scope_tags", "identifiers", "identifier_profile", "version"] {
            if indexes.get(field) != Some(&(PayloadSchemaType::Keyword as i32)) {
                return Ok(Err(Unverified::Search {
                    reason: format!("the keyword index for {field} is missing"),
                }));
            }
        }

        for batch in chunks.chunks(64) {
            let inputs: Vec<String> = batch
                .iter()
                .map(|chunk| self.input(chunk))
                .collect::<Result<_, _>>()?;
            if let Err(error) = record_batch(self.database, self.scopes, &set.id, batch, &inputs) {
                return match verification_write_error(error) {
                    Ok(reason) => Ok(Err(reason)),
                    Err(error) => Err(error),
                };
            }
            let expected: HashMap<String, (&Chunk, Vec<String>)> = batch
                .iter()
                .zip(&inputs)
                .map(|(chunk, input)| (point_id(&chunk.id), (chunk, index_identifiers(input))))
                .collect();
            let point_ids: Vec<String> = expected.keys().cloned().collect();
            let points = self
                .qdrant
                .payload_points(&super::collection_name(generation), &point_ids)
                .await
                .map_err(Error::Qdrant)?;
            if points.len() != batch.len() {
                return Ok(Err(Unverified::Search {
                    reason: "a published point is missing its search payload".to_owned(),
                }));
            }
            if let Err(reason) = check_payloads(points, expected) {
                return Ok(Err(Unverified::Search { reason }));
            }
        }
        if generation.state == GenerationState::Building {
            self.database
                .complete_generation_search(self.scopes, generation.id)
                .map_err(Error::Search)?;
        }
        Ok(Ok(()))
    }
}

/// Maps immutable derivative conflicts to a verification failure, not a retry.
fn verification_write_error(error: Error) -> Result<Unverified, Error> {
    match error {
        Error::Search(
            error @ (RetrievalError::InputConflict | RetrievalError::MembershipConflict),
        ) => Ok(Unverified::Search {
            reason: error.to_string(),
        }),
        error => Err(error),
    }
}

/// Checks that each retrieved point's stable ID and exact search payload
/// match the owning kernel chunk and its prepared input.
fn check_payloads(
    points: Vec<RetrievedPoint>,
    mut expected: HashMap<String, (&Chunk, Vec<String>)>,
) -> Result<(), String> {
    for point in points {
        let Some(id) = point
            .id
            .as_ref()
            .and_then(|id| id.point_id_options.as_ref())
        else {
            return Err("a published search point has no point ID".to_owned());
        };
        let id = match id {
            PointIdOptions::Uuid(id) => id.clone(),
            PointIdOptions::Num(id) => id.to_string(),
        };
        let Some((chunk, identifiers)) = expected.remove(&id) else {
            return Err("a published search point has an unexpected point ID".to_owned());
        };
        if payload_text(&point, "chunk_id") != Some(chunk.id.as_str())
            || payload_text(&point, "revision_id") != Some(chunk.revision_id.as_str())
        {
            return Err("a published search point has inconsistent chunk ownership".to_owned());
        }
        if payload_text(&point, "identifier_profile") != Some(PROFILE) {
            return Err("a published search point has the wrong identifier profile".to_owned());
        }
        if payload_identifiers(&point).as_deref() != Some(identifiers.as_slice()) {
            return Err("a published search point has inconsistent identifiers".to_owned());
        }
    }
    if expected.is_empty() {
        Ok(())
    } else {
        Err("a published search point is missing".to_owned())
    }
}

/// Reads one string payload field.
fn payload_text<'a>(point: &'a RetrievedPoint, field: &str) -> Option<&'a str> {
    match point
        .payload
        .get(field)
        .and_then(|value| value.kind.as_ref())
    {
        Some(Kind::StringValue(value)) => Some(value),
        _ => None,
    }
}

/// Reads a payload keyword array, refusing any non-string item.
fn payload_identifiers(point: &RetrievedPoint) -> Option<Vec<String>> {
    let Some(Kind::ListValue(identifiers)) = point
        .payload
        .get("identifiers")
        .and_then(|value| value.kind.as_ref())
    else {
        return None;
    };
    identifiers
        .values
        .iter()
        .map(|value| match value.kind.as_ref() {
            Some(Kind::StringValue(value)) => Some(value.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Error, RetrievalError, Unverified, verification_write_error};

    #[test]
    fn deterministic_search_write_conflicts_are_unverified() {
        for error in [
            RetrievalError::InputConflict,
            RetrievalError::MembershipConflict,
        ] {
            assert!(matches!(
                verification_write_error(Error::Search(error)),
                Ok(Unverified::Search { .. })
            ));
        }
    }
}
