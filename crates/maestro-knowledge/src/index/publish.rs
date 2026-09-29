//! Publishing a chunk set as a generation: found again or created, built in
//! its own collection batch by batch, checked, and only then behind the
//! collection's alias.

use super::{
    batches::{BATCH, Target},
    dense::embedding_profile,
    error::{Error, Unverified},
    progress::{Progress, Report},
    projection::{Projection, ProjectionWithBatchSize},
    publication_names::{Names, report},
    search_inputs,
    verify::{vectors, verify},
};
use crate::{lexical, query::PROFILE};
use maestro_kernel::{
    chunk_set::{Chunk, ChunkSet, ChunkSetState},
    gateway::{ModelPort, Role},
    generation::{Generation, GenerationState, NewGeneration},
    telemetry::{
        span,
        stage::{Count, Outcome, Stage},
    },
};
use std::{future::Future, ops::ControlFlow};

/// Where a generation found again stands, and what its publication still
/// does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    /// It is building: write its points, check it, then publish it.
    Build,
    /// It is verified: check it again, then publish it.
    Check,
    /// It is published: nothing is left to do.
    Done,
}

impl Step {
    /// What is left to do for a generation in `state`; none once it is
    /// retired or failed, which is never published again.
    pub(super) fn of(state: GenerationState) -> Option<Self> {
        match state {
            GenerationState::Building => Some(Self::Build),
            GenerationState::Verified => Some(Self::Check),
            GenerationState::Published => Some(Self::Done),
            GenerationState::Retired | GenerationState::Failed => None,
        }
    }
}

impl<
    P: ModelPort,
    R: super::projection_port::RetrievalProjectionPort<
            Error = super::projection_port::ProjectionError,
        >,
> Projection<'_, P, R>
{
    /// Publishes the complete chunk set `chunk_set` as a generation of its
    /// collection, and returns its report: [`Projection::publish_observed`],
    /// resuming nothing and observed by no one.
    ///
    /// # Errors
    ///
    /// As [`Projection::publish_observed`].
    pub async fn publish(&self, chunk_set: &str) -> Result<Report, Error> {
        let mut unobserved = |_: &Progress| ControlFlow::Continue(());
        self.publish_observed(chunk_set, None, &mut unobserved)
            .await
    }

    /// Publishes the complete chunk set `chunk_set` as a generation of its
    /// collection, and returns its report (plan D9).
    ///
    /// The generation is the last of its collection built from the same
    /// chunk set with the same profiles that is not retired or failed, or a
    /// new one, building. Its collection in Qdrant, `maestro-<collection>-
    /// g<n>`, is created with a dense vector `dense` of the card's
    /// dimensions compared by cosine and a sparse vector `bm25` weighted by
    /// IDF. Its chunks are then represented and written in batches, in record
    /// order, from the first after those `resume` says its collection holds,
    /// when `resume` is a step of this generation; `observer` sees the
    /// progress once each batch is written, which a job journals. Once every
    /// chunk is written, the collection is checked, the generation verified
    /// with its point count, the alias `maestro-<collection>` moved to its
    /// collection in one action, and the generation published in the kernel,
    /// which retires the one published before it. A verified generation is
    /// checked again and published; a published one is reported as it is
    /// only while its Qdrant collection exists. It reads the kernel
    /// synchronously between awaits; callers run it on a runtime of their own,
    /// as `prepare/bridge.rs` does.
    ///
    /// # Errors
    ///
    /// Before any work, [`Error::NotAnEmbedder`], [`Error::UnknownChunkSet`]
    /// and [`Error::Incomplete`]. Part way, [`Error::Embedding`],
    /// [`Error::Unreadable`], [`Error::Projection`] and [`Error::Stopped`], and
    /// the kernel's failures, all of which leave the generation as it was, so
    /// a rerun resumes it; [`Error::MissingCollection`] when a published
    /// generation's collection is gone; and [`Error::Unverified`] when a
    /// collection fails a check, which fails the generation. A failed
    /// re-check restores the alias to the previous published generation when
    /// its collection still exists.
    pub async fn publish_observed(
        &self,
        chunk_set: &str,
        resume: Option<&Progress>,
        observer: &mut impl FnMut(&Progress) -> ControlFlow<()>,
    ) -> Result<Report, Error> {
        self.publish_observed_with_batch_size(chunk_set, resume, observer, BATCH)
            .await
    }

    /// Publishes the set in batches of `batch_size` chunks, traced as a
    /// `knowledge.publish` stage whose steps are its child stages.
    pub(super) async fn publish_observed_with_batch_size(
        &self,
        chunk_set: &str,
        resume: Option<&Progress>,
        observer: &mut impl FnMut(&Progress) -> ControlFlow<()>,
        batch_size: usize,
    ) -> Result<Report, Error> {
        let stage = span::publish();
        let result = stage
            .instrument(Box::pin(
                self.publish_steps(chunk_set, resume, observer, batch_size),
            ))
            .await;
        if let Ok(report) = &result {
            stage.collection(&report.collection);
            stage.generation(report.generation);
            stage.count(
                Count::Points,
                usize::try_from(report.points).unwrap_or(usize::MAX),
            );
        }
        stage.finish(result.as_ref().map_or_else(Error::outcome, |_| Outcome::Ok));
        result
    }

    /// Publishes the set in batches of `batch_size` chunks, each step as its
    /// stage.
    async fn publish_steps(
        &self,
        chunk_set: &str,
        resume: Option<&Progress>,
        observer: &mut impl FnMut(&Progress) -> ControlFlow<()>,
        batch_size: usize,
    ) -> Result<Report, Error> {
        let (dimensions, set, generation, step) = traced_step(span::publish_load_inputs(), async {
            let dimensions = self.dimensions()?;
            let set = self.complete(chunk_set)?;
            self.check_counter_contract(&set)?;
            let (generation, step) = self.generation(&set)?;
            Ok((dimensions, set, generation, step))
        })
        .await?;
        let names = Names::of(&generation);
        if step == Step::Done {
            if !self
                .projection
                .collection_exists(&names.collection)
                .await
                .map_err(Error::from)?
            {
                return Err(Error::MissingCollection(names.collection));
            }
            let points = generation.point_count.unwrap_or_default();
            return Ok(report(&set, &generation, &names, points, None));
        }
        let chunks = self
            .database
            .chunks(self.scopes, &set.id)
            .map_err(Error::ChunkSet)?;
        if step == Step::Build {
            traced_step(span::publish_project(), async {
                let new_search_profile =
                    search_inputs::begin(self.database, self.scopes, &generation)?;
                search_inputs::record_members(self.database, self.scopes, &set)?;
                let created = self.ensure(&names, dimensions).await?;
                let start = start(created || new_search_profile, resume, generation.id);
                let target = Target {
                    collection: &names.collection,
                    generation: generation.id,
                    chunk_set_id: &set.id,
                    chunks: &chunks,
                };
                self.index(&target, start, batch_size, observer).await
            })
            .await?;
        }
        let points = traced_step(
            span::publish_verify(),
            self.verify_steps(&names, dimensions, (&set, &generation, step), &chunks),
        )
        .await?;
        traced_step(span::publish_switch_alias(), async {
            self.projection
                .replace_alias(&names.alias, &names.collection)
                .await
                .map_err(Error::from)
        })
        .await?;
        let retired = traced_step(span::publish_kernel(), async {
            self.database
                .publish_generation(generation.id)
                .map_err(Error::Generation)
        })
        .await?;
        Ok(report(&set, &generation, &names, points, retired))
    }

    /// The points of the collection of `names` once it passes every check,
    /// the search check among them, and the generation is verified.
    async fn verify_steps(
        &self,
        names: &Names,
        dimensions: u64,
        (set, generation, step): (&ChunkSet, &Generation, Step),
        chunks: &[Chunk],
    ) -> Result<u64, Error> {
        let rollback = if step == Step::Check {
            self.database
                .published_generation(self.scopes, &set.collection_id)
                .map_err(Error::Generation)?
                .map(|published| Names::of(&published).collection)
        } else {
            None
        };
        let points = self
            .check(names, dimensions, chunks, rollback.as_deref())
            .await?;
        if let Err(reason) = self.verify_search(generation, set, chunks).await? {
            let rollback = rollback
                .as_deref()
                .map(|collection| (names.alias.as_str(), collection));
            return self.fail(names.generation, reason, rollback).await;
        }
        if step == Step::Build {
            self.database
                .verify_generation(generation.id, points)
                .map_err(Error::Generation)?;
        }
        Ok(points)
    }

    /// Prevents a v2 card from reusing chunks counted for another identity.
    pub(super) fn check_counter_contract(&self, set: &ChunkSet) -> Result<(), Error> {
        if self.card.identity().is_none() {
            return Ok(());
        }
        let expected = format!("router/1:sha256:{}", self.card.digest().as_str());
        if set.counter_contract_id == expected {
            Ok(())
        } else {
            Err(Error::CounterContractMismatch {
                expected,
                actual: set.counter_contract_id.clone(),
            })
        }
    }

    /// The card's dimensions, when it is an embedder's.
    pub(super) fn dimensions(&self) -> Result<u64, Error> {
        match (self.card.fields().role, self.card.fields().dimensions) {
            (Role::Embedder, Some(dimensions)) => {
                Ok(u64::try_from(dimensions.get()).unwrap_or(u64::MAX))
            }
            (role, _) => Err(Error::NotAnEmbedder {
                card: self.card.digest().clone(),
                role,
            }),
        }
    }

    /// The chunk set `id`, when the caller reads it and it is complete.
    pub(super) fn complete(&self, id: &str) -> Result<ChunkSet, Error> {
        let set = self
            .database
            .chunk_set(self.scopes, id)
            .map_err(Error::ChunkSet)?
            .ok_or_else(|| Error::UnknownChunkSet(id.to_owned()))?;
        match set.state {
            ChunkSetState::Complete => Ok(set),
            state => Err(Error::Incomplete {
                chunk_set: set.id,
                state,
            }),
        }
    }

    /// The generation that publishes `set` with this projection's profiles,
    /// and what is left to do for it: the last of its collection so built
    /// that is not retired or failed, or a new one, building.
    fn generation(&self, set: &ChunkSet) -> Result<(Generation, Step), Error> {
        let embedding = embedding_profile(self.card);
        let generations = self
            .database
            .generations(self.scopes, &set.collection_id)
            .map_err(Error::Generation)?;
        for generation in generations.into_iter().rev().filter(|generation| {
            generation.chunk_set_id == set.id
                && generation.embedding_profile == embedding
                && generation.sparse_profile == lexical::PROFILE
        }) {
            let Some(step) = Step::of(generation.state) else {
                continue;
            };
            let projection = self
                .database
                .generation_search(self.scopes, generation.id)
                .map_err(Error::Search)?;
            let reusable = match step {
                Step::Build => projection
                    .as_ref()
                    .is_none_or(|projection| projection.identifier_profile == PROFILE),
                Step::Check | Step::Done => projection.as_ref().is_some_and(|projection| {
                    projection.identifier_profile == PROFILE && projection.ready
                }),
            };
            if reusable {
                return Ok((generation, step));
            }
        }
        let created = self
            .database
            .create_generation(&NewGeneration {
                collection_id: set.collection_id.clone(),
                chunk_set_id: set.id.clone(),
                embedding_profile: embedding,
                sparse_profile: lexical::PROFILE.to_owned(),
            })
            .map_err(Error::Generation)?;
        Ok((created, Step::Build))
    }

    /// Creates the collection of `names`, unless it exists already, when its
    /// vectors must be those of a generation whose embedder gives
    /// `dimensions`: a collection with others fails the generation.
    pub(super) async fn ensure(&self, names: &Names, dimensions: u64) -> Result<bool, Error> {
        let collection = &names.collection;
        let layout = self
            .projection
            .collection_layout(collection)
            .await
            .map_err(Error::from)?;
        let created = if let Some(layout) = layout {
            if let Err(reason) = vectors(&layout, dimensions) {
                return self.fail(names.generation, reason, None).await;
            }
            false
        } else {
            self.projection
                .create_collection(
                    collection,
                    super::projection_port::CollectionLayout {
                        dense_dimensions: dimensions,
                        dense_present: true,
                        dense_distance: "Cosine".to_owned(),
                        sparse_present: true,
                        sparse_modifier: Some("Idf".to_owned()),
                    },
                )
                .await
                .map_err(Error::from)?;
            true
        };
        self.projection
            .index_payload_fields(collection)
            .await
            .map_err(Error::from)?;
        Ok(created)
    }

    /// The points the collection of `names` holds once it passes every
    /// check of a generation whose embedder gives `dimensions` and whose
    /// chunk set holds `chunks`; a check it fails fails the generation.
    pub(super) async fn check(
        &self,
        names: &Names,
        dimensions: u64,
        chunks: &[Chunk],
        rollback: Option<&str>,
    ) -> Result<u64, Error> {
        match verify(self.projection, &names.collection, dimensions, chunks)
            .await
            .map_err(Error::from)?
        {
            Ok(points) => Ok(points),
            Err(reason) => {
                let rollback = rollback.map(|collection| (names.alias.as_str(), collection));
                self.fail(names.generation, reason, rollback).await
            }
        }
    }

    /// Fails the generation `generation` for `reason`, and refuses its
    /// publication. A failed re-check restores the alias to `rollback` when
    /// that published collection still exists.
    pub(super) async fn fail<T>(
        &self,
        generation: i64,
        reason: Unverified,
        rollback: Option<(&str, &str)>,
    ) -> Result<T, Error> {
        self.database
            .fail_generation(generation)
            .map_err(Error::Generation)?;
        if let Some((alias, collection)) = rollback
            && self
                .projection
                .collection_exists(collection)
                .await
                .map_err(Error::from)?
        {
            self.projection
                .replace_alias(alias, collection)
                .await
                .map_err(Error::from)?;
        }
        Err(Error::Unverified { generation, reason })
    }
}

impl<
    P: ModelPort,
    R: super::projection_port::RetrievalProjectionPort<
            Error = super::projection_port::ProjectionError,
        >,
> ProjectionWithBatchSize<'_, P, R>
{
    /// Publishes `chunk_set` with the test-selected batch size, without
    /// observing progress.
    ///
    /// # Errors
    ///
    /// As [`Projection::publish`].
    pub async fn publish(&self, chunk_set: &str) -> Result<Report, Error> {
        let mut unobserved = |_: &Progress| ControlFlow::Continue(());
        self.publish_observed(chunk_set, None, &mut unobserved)
            .await
    }

    /// Publishes `chunk_set` with the test-selected batch size and observes
    /// progress after each written batch.
    ///
    /// # Errors
    ///
    /// As [`Projection::publish_observed`].
    pub async fn publish_observed(
        &self,
        chunk_set: &str,
        resume: Option<&Progress>,
        observer: &mut impl FnMut(&Progress) -> ControlFlow<()>,
    ) -> Result<Report, Error> {
        self.projection
            .publish_observed_with_batch_size(chunk_set, resume, observer, self.batch_size.get())
            .await
    }
}

/// Where the build of the generation `generation` starts: from its first
/// chunk when its collection or search profile is `fresh`, else after the
/// chunks `resume` says its collection holds, when `resume` is its step.
fn start(fresh: bool, resume: Option<&Progress>, generation: i64) -> u64 {
    if fresh {
        return 0;
    }
    resume
        .filter(|progress| progress.generation == generation)
        .map_or(0, |progress| progress.indexed)
}

/// Runs one step of a publication as the stage `stage`, and records how it
/// ended.
async fn traced_step<T>(
    stage: Stage,
    work: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    let result = stage.instrument(work).await;
    stage.finish(result.as_ref().map_or_else(Error::outcome, |_| Outcome::Ok));
    result
}
