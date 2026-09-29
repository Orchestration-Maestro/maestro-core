//! The kernel-owned search derivatives of one published generation.

use super::{
    error::{Error, Unverified},
    names::collection_name,
    payload_text,
    point::point_id,
    projection::Projection,
    projection_port::{PointHit, RetrievalProjectionPort},
};
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

impl<P: ModelPort, R: RetrievalProjectionPort> Projection<'_, P, R> {
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
            .projection
            .payload_fields(&collection_name(generation))
            .await
            .map_err(Error::from)?;
        for field in ["scope_tags", "identifiers", "identifier_profile", "version"] {
            if indexes.get(field).map(String::as_str) != Some("keyword") {
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
                .projection
                .payloads(&collection_name(generation), &point_ids)
                .await
                .map_err(Error::from)?;
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
    points: Vec<PointHit>,
    mut expected: HashMap<String, (&Chunk, Vec<String>)>,
) -> Result<(), String> {
    for point in points {
        if point.id.is_empty() {
            return Err("a published search point has no point ID".to_owned());
        }
        let id = point.id.clone();
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

/// Reads a payload keyword array, refusing any non-string item.
fn payload_identifiers(point: &PointHit) -> Option<Vec<String>> {
    point
        .payload
        .get("identifiers")?
        .as_array()?
        .iter()
        .map(|value| value.as_str().map(str::to_owned))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        Error, PROFILE, RetrievalError, Unverified, check_payloads, point_id, record_batch,
        verification_write_error,
    };
    use crate::index::projection_port::PointHit;
    use crate::prepare::tests::scratch::Scratch;
    use maestro_kernel::{
        artifact::Digest,
        chunk_set::Chunk,
        evidence::Span,
        scope::{Right, Scope},
    };
    use serde_json::{Value, json};
    use std::collections::HashMap;

    fn chunk() -> Chunk {
        Chunk {
            id: "chunk-0-1".to_owned(),
            revision_id: "revision-1".to_owned(),
            section_id: None,
            digest: Digest::of(b"input"),
            token_count: 1,
            span: Span { start: 0, end: 1 },
        }
    }

    fn payload(chunk: &str, revision: &str, profile: &str, identifiers: &Value) -> Value {
        json!({
            "chunk_id": chunk,
            "revision_id": revision,
            "identifier_profile": profile,
            "identifiers": identifiers,
        })
    }

    fn point(id: &str, payload: Value) -> PointHit {
        PointHit {
            id: id.to_owned(),
            score: None,
            payload: match payload {
                Value::Object(payload) => payload.into_iter().collect(),
                _ => panic!("test payload must be an object"),
            },
        }
    }

    fn expected<'a>(chunk: &'a Chunk, id: &str) -> HashMap<String, (&'a Chunk, Vec<String>)> {
        HashMap::from([(id.to_owned(), (chunk, vec!["ERR-42".to_owned()]))])
    }

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

    #[test]
    fn record_batch_refuses_mismatched_chunk_and_input_counts() {
        let scratch = Scratch::new();
        let database = scratch.database();
        let workspace: Scope = "workspace/default".parse().unwrap();
        database
            .grant("reader", &workspace, Right::Read, "test")
            .unwrap();
        let scopes = database.visible("reader").unwrap();
        let chunk = chunk();
        assert!(matches!(
            record_batch(&database, &scopes, "set", &[chunk], &[]),
            Err(Error::Search(RetrievalError::InvalidInput(message)))
                if message == "a search-input batch has mismatched chunks and prepared inputs"
        ));
    }

    #[test]
    fn search_payload_checks_report_missing_unexpected_and_inconsistent_points() {
        let chunk = chunk();
        let id = point_id(&chunk.id);
        let good = payload(&chunk.id, &chunk.revision_id, PROFILE, &json!(["ERR-42"]));
        assert!(check_payloads(vec![point(&id, good.clone())], expected(&chunk, &id)).is_ok());

        assert_eq!(
            check_payloads(vec![point("", good.clone())], expected(&chunk, &id)).unwrap_err(),
            "a published search point has no point ID"
        );
        assert!(
            check_payloads(
                vec![point("unexpected", good.clone())],
                expected(&chunk, &id)
            )
            .unwrap_err()
            .contains("unexpected point ID")
        );
        assert!(
            check_payloads(Vec::new(), expected(&chunk, &id))
                .unwrap_err()
                .contains("missing")
        );
        assert!(
            check_payloads(
                vec![point(
                    &id,
                    payload(
                        "another-chunk",
                        &chunk.revision_id,
                        PROFILE,
                        &json!(["ERR-42"])
                    ),
                )],
                expected(&chunk, &id)
            )
            .unwrap_err()
            .contains("inconsistent chunk ownership")
        );
        assert!(
            check_payloads(
                vec![point(
                    &id,
                    payload(
                        &chunk.id,
                        &chunk.revision_id,
                        "another-profile",
                        &json!(["ERR-42"])
                    ),
                )],
                expected(&chunk, &id)
            )
            .unwrap_err()
            .contains("wrong identifier profile")
        );
        assert!(
            check_payloads(
                vec![point(
                    &id,
                    payload(&chunk.id, &chunk.revision_id, PROFILE, &json!("not-a-list")),
                )],
                expected(&chunk, &id)
            )
            .unwrap_err()
            .contains("inconsistent identifiers")
        );
        assert!(
            check_payloads(
                vec![point(
                    &id,
                    payload(
                        &chunk.id,
                        &chunk.revision_id,
                        PROFILE,
                        &json!(["ERR-42", 3])
                    ),
                )],
                expected(&chunk, &id)
            )
            .unwrap_err()
            .contains("inconsistent identifiers")
        );
    }

    #[test]
    fn search_payload_check_accepts_numeric_point_ids() {
        let chunk = chunk();
        let payload = payload(&chunk.id, &chunk.revision_id, PROFILE, &json!(["ERR-42"]));
        assert!(check_payloads(vec![point("42", payload)], expected(&chunk, "42")).is_ok());
    }
}
