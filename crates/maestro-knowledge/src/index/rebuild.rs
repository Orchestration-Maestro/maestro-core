//! A fresh, guarded replacement for a published generation whose projection
//! is lost, using the original chunk set and card.

use super::{
    batches::{BATCH, Target},
    dense::embedding_profile,
    error::Error,
    names::collection_name,
    progress::{Progress, Report},
    projection::Projection,
    publish::{Names, Step, report},
    search_inputs,
    verify::verify,
};
use crate::{lexical, query::PROFILE};
use maestro_kernel::{
    chunk_set::{Chunk, ChunkSet},
    gateway::ModelPort,
    generation::{Error as GenerationError, Generation, GenerationState, NewGeneration},
};
use std::ops::ControlFlow;

/// The published pointer and generation boundary frozen when a rebuild starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RebuildGuard {
    /// The published generation at admission, or none.
    pub expected_published: Option<i64>,
    /// Generations at or before this ID cannot be adopted by this rebuild.
    pub generation_watermark: i64,
}

/// Frozen database and Qdrant state for one guarded replacement.
struct RebuildState<'a> {
    /// Dimensions of the card's dense vectors.
    dimensions: u64,
    /// Complete chunk set to rebuild.
    set: ChunkSet,
    /// Embedding profile expected from the restored card.
    expected_embedding: String,
    /// Chunks whose points the replacement must hold.
    chunks: Vec<Chunk>,
    /// Published generation observed while admitting the replacement.
    published: Option<Generation>,
    /// Generation named by journaled progress, if resuming.
    resume_generation: Option<Generation>,
    /// Alias targets this replacement may reconcile.
    allowed_aliases: [Option<String>; 3],
    /// Published pointer and generation watermark frozen by the job.
    guard: RebuildGuard,
    /// Last progress event recorded by the job, if resuming.
    resume: Option<&'a Progress>,
}

impl RebuildState<'_> {
    /// The generation ID observed as published during admission.
    fn published_id(&self) -> Option<i64> {
        self.published.as_ref().map(|generation| generation.id)
    }
}

impl<P: ModelPort> Projection<'_, P> {
    /// Publishes an explicitly requested replacement, retaining the
    /// generation captured by `guard` as its only legal predecessor.
    /// Journaled progress in `resume` continues the same replacement.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PublishedChanged`] if the guarded pointer changes,
    /// [`Error::RecoveryTarget`] for a target that no longer matches its
    /// frozen identity, or an error from the kernel, Qdrant, or stopped observer.
    pub async fn republish_observed(
        &self,
        chunk_set: &str,
        guard: RebuildGuard,
        resume: Option<&Progress>,
        observer: &mut impl FnMut(&Progress) -> ControlFlow<()>,
    ) -> Result<Report, Error> {
        let state = self.load_rebuild_state(chunk_set, guard, resume).await?;
        let (generation, step) = self.select_rebuild_target(&state, observer)?;
        self.validate_search_profile(&generation, step)?;
        let names = Names::of(&generation);
        if step == Step::Done {
            return self.reconcile_completed(&state, &generation, &names).await;
        }
        if state.published_id() != guard.expected_published {
            return Err(Error::PublishedChanged {
                expected: guard.expected_published,
                found: state.published_id(),
            });
        }
        self.finish_rebuild(&state, &generation, step, observer)
            .await
    }

    /// Reads the frozen tuple and validates its current generation and alias.
    async fn load_rebuild_state<'a>(
        &self,
        chunk_set: &str,
        guard: RebuildGuard,
        resume: Option<&'a Progress>,
    ) -> Result<RebuildState<'a>, Error> {
        let dimensions = self.dimensions()?;
        let set = self.complete(chunk_set)?;
        self.check_counter_contract(&set)?;
        let expected_embedding = embedding_profile(self.card);
        let chunks = self
            .database
            .chunks(self.scopes, &set.id)
            .map_err(Error::ChunkSet)?;
        let published = self
            .database
            .published_generation(self.scopes, &set.collection_id)
            .map_err(Error::Generation)?;
        let published_id = published.as_ref().map(|generation| generation.id);
        let resume_generation = resume
            .map(|progress| {
                self.database
                    .generation(self.scopes, progress.generation)
                    .map_err(Error::Generation)?
                    .ok_or(Error::RecoveryTarget {
                        generation: progress.generation,
                    })
            })
            .transpose()?;
        let resume_published = resume_generation.as_ref().is_some_and(|generation| {
            generation.state == GenerationState::Published && published_id == Some(generation.id)
        });
        if published_id != guard.expected_published && !resume_published {
            return Err(Error::PublishedChanged {
                expected: guard.expected_published,
                found: published_id,
            });
        }
        if let Some(current) = &published {
            self.projection
                .exists(&collection_name(current))
                .await
                .map_err(Error::Qdrant)?;
        }

        let alias = format!("maestro-{}", set.collection_id);
        let expected_alias = match guard.expected_published {
            Some(id) => self
                .database
                .generation(self.scopes, id)
                .map_err(Error::Generation)?
                .map(|generation| collection_name(&generation)),
            None => None,
        };
        let allowed_aliases = [
            published.as_ref().map(collection_name),
            resume_generation.as_ref().map(collection_name),
            expected_alias,
        ];
        let alias_target = self
            .projection
            .alias_collection(&alias)
            .await
            .map_err(Error::Qdrant)?;
        if let Some(found) = &alias_target
            && !allowed_aliases
                .iter()
                .flatten()
                .any(|allowed| allowed == found)
        {
            return Err(Error::UnrelatedAlias {
                alias,
                found: found.clone(),
            });
        }

        Ok(RebuildState {
            dimensions,
            set,
            expected_embedding,
            chunks,
            published,
            resume_generation,
            allowed_aliases,
            guard,
            resume,
        })
    }

    /// Resumes journaled work or selects a post-watermark generation.
    fn select_rebuild_target(
        &self,
        state: &RebuildState<'_>,
        observer: &mut impl FnMut(&Progress) -> ControlFlow<()>,
    ) -> Result<(Generation, Step), Error> {
        if let Some(progress) = state.resume {
            let generation = state
                .resume_generation
                .clone()
                .ok_or(Error::RecoveryTarget {
                    generation: progress.generation,
                })?;
            validate_target(&generation, state, progress)?;
            let step = Step::of(generation.state).ok_or(Error::RecoveryTarget {
                generation: generation.id,
            })?;
            return Ok((generation, step));
        }

        let target = self
            .unfinished_target(&state.set, state.guard.generation_watermark)?
            .map_or_else(
                || {
                    self.database
                        .create_generation(&NewGeneration {
                            collection_id: state.set.collection_id.clone(),
                            chunk_set_id: state.set.id.clone(),
                            embedding_profile: state.expected_embedding.clone(),
                            sparse_profile: lexical::PROFILE.to_owned(),
                        })
                        .map_err(Error::Generation)
                },
                Ok,
            )?;
        validate_tuple(
            &target,
            &state.set,
            &state.expected_embedding,
            state.guard.generation_watermark,
        )?;
        let step = Step::of(target.state).ok_or(Error::RecoveryTarget {
            generation: target.id,
        })?;
        let initial = Progress {
            generation: target.id,
            indexed: 0,
            chunks: u64::try_from(state.chunks.len()).unwrap_or(u64::MAX),
            average_length: 0.0,
        };
        if observer(&initial).is_break() {
            return Err(Error::Stopped);
        }
        Ok((target, step))
    }

    /// Confirms the target's identifier projection belongs to this profile.
    fn validate_search_profile(&self, generation: &Generation, step: Step) -> Result<(), Error> {
        let projection = self
            .database
            .generation_search(self.scopes, generation.id)
            .map_err(Error::Search)?;
        let valid = match step {
            Step::Build => projection
                .as_ref()
                .is_none_or(|projection| projection.identifier_profile == PROFILE),
            Step::Check | Step::Done => projection.as_ref().is_some_and(|projection| {
                projection.identifier_profile == PROFILE && projection.ready
            }),
        };
        if valid {
            Ok(())
        } else {
            Err(Error::RecoveryTarget {
                generation: generation.id,
            })
        }
    }

    /// Rechecks and reports a target whose progress says it is published.
    async fn reconcile_completed(
        &self,
        state: &RebuildState<'_>,
        generation: &Generation,
        names: &Names,
    ) -> Result<Report, Error> {
        if state.published_id() != Some(generation.id) {
            return Err(Error::PublishedChanged {
                expected: Some(generation.id),
                found: state.published_id(),
            });
        }
        if !self
            .projection
            .exists(&names.collection)
            .await
            .map_err(Error::Qdrant)?
        {
            return Err(Error::MissingCollection(names.collection.clone()));
        }
        let points = match verify(
            self.projection,
            &names.collection,
            state.dimensions,
            &state.chunks,
        )
        .await
        .map_err(Error::Qdrant)?
        {
            Ok(points) => points,
            Err(reason) => {
                return Err(Error::Unverified {
                    generation: generation.id,
                    reason,
                });
            }
        };
        if let Err(reason) = self
            .verify_search(generation, &state.set, &state.chunks)
            .await?
        {
            return Err(Error::Unverified {
                generation: generation.id,
                reason,
            });
        }
        if self
            .projection
            .alias_collection(&names.alias)
            .await
            .map_err(Error::Qdrant)?
            .as_deref()
            != Some(names.collection.as_str())
        {
            self.projection
                .point_alias(&names.alias, &names.collection)
                .await
                .map_err(Error::Qdrant)?;
        }
        Ok(report(
            &state.set,
            generation,
            names,
            points,
            state.guard.expected_published,
        ))
    }

    /// Builds, checks, and compare-and-publishes a replacement target.
    async fn finish_rebuild(
        &self,
        state: &RebuildState<'_>,
        generation: &Generation,
        step: Step,
        observer: &mut impl FnMut(&Progress) -> ControlFlow<()>,
    ) -> Result<Report, Error> {
        let new_search_profile = if step == Step::Build {
            let new = search_inputs::begin(self.database, self.scopes, generation)?;
            search_inputs::record_members(self.database, self.scopes, &state.set)?;
            new
        } else {
            false
        };
        let names = Names::of(generation);
        let created = self.ensure(&names, state.dimensions).await?;
        if step == Step::Build || created {
            let start = if created || new_search_profile {
                0
            } else {
                state
                    .resume
                    .filter(|progress| progress.generation == generation.id)
                    .map_or(0, |progress| progress.indexed)
            };
            let target = Target {
                collection: &names.collection,
                generation: generation.id,
                chunk_set_id: &state.set.id,
                chunks: &state.chunks,
            };
            self.index(&target, start, BATCH, observer).await?;
        }

        let rollback = state.guard.expected_published.and_then(|id| {
            state
                .published
                .as_ref()
                .filter(|published| published.id == id)
                .map(collection_name)
        });
        let points = self
            .check(&names, state.dimensions, &state.chunks, rollback.as_deref())
            .await?;
        if let Err(reason) = self
            .verify_search(generation, &state.set, &state.chunks)
            .await?
        {
            let rollback = rollback
                .as_deref()
                .map(|collection| (names.alias.as_str(), collection));
            return self.fail(generation.id, reason, rollback).await;
        }
        if step == Step::Build {
            self.database
                .verify_generation(generation.id, points)
                .map_err(Error::Generation)?;
        }
        self.commit_rebuild(state, generation, &names, points).await
    }

    /// Moves the alias and atomically commits the guarded generation pointer.
    async fn commit_rebuild(
        &self,
        state: &RebuildState<'_>,
        generation: &Generation,
        names: &Names,
        points: u64,
    ) -> Result<Report, Error> {
        let current = self
            .database
            .published_generation(self.scopes, &state.set.collection_id)
            .map_err(Error::Generation)?
            .map(|generation| generation.id);
        if current != state.guard.expected_published {
            return Err(Error::PublishedChanged {
                expected: state.guard.expected_published,
                found: current,
            });
        }
        self.projection
            .point_alias(&names.alias, &names.collection)
            .await
            .map_err(Error::Qdrant)?;
        let alias_target = self
            .projection
            .alias_collection(&names.alias)
            .await
            .map_err(Error::Qdrant)?;
        if let Some(found) = alias_target
            && found != names.collection
            && !state
                .allowed_aliases
                .iter()
                .flatten()
                .any(|allowed| allowed == &found)
        {
            return Err(Error::UnrelatedAlias {
                alias: names.alias.clone(),
                found,
            });
        }

        let retired = match self
            .database
            .publish_generation_if_current(generation.id, state.guard.expected_published)
        {
            Ok(retired) => retired,
            Err(GenerationError::PublishedChanged { expected, found }) => {
                if let Some(current) = self
                    .database
                    .published_generation(self.scopes, &state.set.collection_id)
                    .map_err(Error::Generation)?
                    && self
                        .projection
                        .exists(&collection_name(&current))
                        .await
                        .map_err(Error::Qdrant)?
                {
                    self.projection
                        .point_alias(&names.alias, &collection_name(&current))
                        .await
                        .map_err(Error::Qdrant)?;
                }
                return Err(Error::PublishedChanged { expected, found });
            }
            Err(error) => return Err(Error::Generation(error)),
        };
        Ok(report(&state.set, generation, names, points, retired))
    }

    /// Finds the unique matching generation created after this recovery job's
    /// frozen watermark. Older work is never adopted by a new job.
    fn unfinished_target(
        &self,
        set: &ChunkSet,
        watermark: i64,
    ) -> Result<Option<Generation>, Error> {
        let embedding = embedding_profile(self.card);
        let mut candidates = self
            .database
            .generations(self.scopes, &set.collection_id)
            .map_err(Error::Generation)?
            .into_iter()
            .filter(|generation| {
                generation.id > watermark
                    && generation.chunk_set_id == set.id
                    && generation.embedding_profile == embedding
                    && generation.sparse_profile == lexical::PROFILE
                    && Step::of(generation.state).is_some()
            })
            .collect::<Vec<_>>();
        match candidates.len() {
            0 => Ok(None),
            1 => Ok(candidates.pop()),
            _ => Err(Error::AmbiguousRecoveryTarget {
                generations: candidates.iter().map(|generation| generation.id).collect(),
            }),
        }
    }
}

/// Confirms a journaled target still matches the explicit recovery tuple.
fn validate_target(
    generation: &Generation,
    state: &RebuildState<'_>,
    progress: &Progress,
) -> Result<(), Error> {
    validate_tuple(
        generation,
        &state.set,
        &state.expected_embedding,
        state.guard.generation_watermark,
    )?;
    let expected_chunks = u64::try_from(state.chunks.len()).unwrap_or(u64::MAX);
    if progress.chunks != expected_chunks || progress.indexed > expected_chunks {
        return Err(Error::RecoveryTarget {
            generation: generation.id,
        });
    }
    Ok(())
}

/// Confirms a candidate belongs to this set, its restored card and profile.
fn validate_tuple(
    generation: &Generation,
    set: &ChunkSet,
    embedding: &str,
    watermark: i64,
) -> Result<(), Error> {
    if generation.id <= watermark
        || generation.collection_id != set.collection_id
        || generation.chunk_set_id != set.id
        || generation.embedding_profile != embedding
        || generation.sparse_profile != lexical::PROFILE
    {
        return Err(Error::RecoveryTarget {
            generation: generation.id,
        });
    }
    Ok(())
}
