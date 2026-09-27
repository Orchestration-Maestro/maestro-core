//! Publishing a chunk set as a generation: found again or created, built in
//! its own collection batch by batch, checked, and only then behind the
//! collection's alias.

use super::{
    batches::Target,
    dense::embedding_profile,
    error::{Error, Unverified},
    names::collection_name,
    progress::{Progress, Report},
    projection::Projection,
    search_inputs,
    verify::{vectors, verify},
};
use crate::{lexical, query::PROFILE};
use maestro_kernel::{
    chunk_set::{Chunk, ChunkSet, ChunkSetState},
    gateway::{ModelPort, Role},
    generation::{Generation, GenerationState, NewGeneration},
};
use std::ops::ControlFlow;

/// Where a generation found again stands, and what its publication still
/// does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
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
    fn of(state: GenerationState) -> Option<Self> {
        match state {
            GenerationState::Building => Some(Self::Build),
            GenerationState::Verified => Some(Self::Check),
            GenerationState::Published => Some(Self::Done),
            GenerationState::Retired | GenerationState::Failed => None,
        }
    }
}

impl<P: ModelPort> Projection<'_, P> {
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
    /// [`Error::Unreadable`], [`Error::Qdrant`] and [`Error::Stopped`], and
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
        let dimensions = self.dimensions()?;
        let set = self.complete(chunk_set)?;
        let (generation, step) = self.generation(&set)?;
        let names = Names::of(&generation);
        if step == Step::Done {
            if !self
                .qdrant
                .exists(&names.collection)
                .await
                .map_err(Error::Qdrant)?
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
            let new_search_profile = search_inputs::begin(self.database, self.scopes, &generation)?;
            search_inputs::record_members(self.database, self.scopes, &set)?;
            let created = self.ensure(&names, dimensions).await?;
            let start = if created || new_search_profile {
                0
            } else {
                resume
                    .filter(|progress| progress.generation == generation.id)
                    .map_or(0, |progress| progress.indexed)
            };
            let target = Target {
                collection: &names.collection,
                generation: generation.id,
                chunk_set_id: &set.id,
                chunks: &chunks,
            };
            self.index(&target, start, observer).await?;
        }
        let rollback = if step == Step::Check {
            self.database
                .published_generation(self.scopes, &set.collection_id)
                .map_err(Error::Generation)?
                .map(|published| Names::of(&published).collection)
        } else {
            None
        };
        let points = self
            .check(&names, dimensions, &chunks, rollback.as_deref())
            .await?;
        if let Err(reason) = self.verify_search(&generation, &set, &chunks).await? {
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
        self.qdrant
            .point_alias(&names.alias, &names.collection)
            .await
            .map_err(Error::Qdrant)?;
        let retired = self
            .database
            .publish_generation(generation.id)
            .map_err(Error::Generation)?;
        Ok(report(&set, &generation, &names, points, retired))
    }

    /// The card's dimensions, when it is an embedder's.
    fn dimensions(&self) -> Result<u64, Error> {
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
    fn complete(&self, id: &str) -> Result<ChunkSet, Error> {
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
    async fn ensure(&self, names: &Names, dimensions: u64) -> Result<bool, Error> {
        let collection = &names.collection;
        let created = if self
            .qdrant
            .exists(collection)
            .await
            .map_err(Error::Qdrant)?
        {
            let parameters = self
                .qdrant
                .parameters(collection)
                .await
                .map_err(Error::Qdrant)?;
            if let Err(reason) = vectors(&parameters, dimensions) {
                return self.fail(names.generation, reason, None).await;
            }
            false
        } else {
            self.qdrant
                .create(collection, dimensions)
                .await
                .map_err(Error::Qdrant)?;
            true
        };
        self.qdrant
            .index_search_fields(collection)
            .await
            .map_err(Error::Qdrant)?;
        Ok(created)
    }

    /// The points the collection of `names` holds once it passes every
    /// check of a generation whose embedder gives `dimensions` and whose
    /// chunk set holds `chunks`; a check it fails fails the generation.
    async fn check(
        &self,
        names: &Names,
        dimensions: u64,
        chunks: &[Chunk],
        rollback: Option<&str>,
    ) -> Result<u64, Error> {
        match verify(self.qdrant, &names.collection, dimensions, chunks)
            .await
            .map_err(Error::Qdrant)?
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
    async fn fail<T>(
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
                .qdrant
                .exists(collection)
                .await
                .map_err(Error::Qdrant)?
        {
            self.qdrant
                .point_alias(alias, collection)
                .await
                .map_err(Error::Qdrant)?;
        }
        Err(Error::Unverified { generation, reason })
    }
}

/// The names of a generation in Qdrant.
#[derive(Debug)]
struct Names {
    /// Its generation ID.
    generation: i64,
    /// Its collection, `maestro-<collection>-g<n>`.
    collection: String,
    /// Its collection's alias, `maestro-<collection>`.
    alias: String,
}

impl Names {
    /// The names of `generation` in Qdrant.
    fn of(generation: &Generation) -> Self {
        Self {
            generation: generation.id,
            collection: collection_name(generation),
            alias: super::alias_name(generation),
        }
    }
}

/// The report of `generation`, which publishes `set` under `names` with
/// `points`, and retired the generation `retired`.
fn report(
    set: &ChunkSet,
    generation: &Generation,
    names: &Names,
    points: u64,
    retired: Option<i64>,
) -> Report {
    Report {
        collection: set.collection_id.clone(),
        chunk_set: set.id.clone(),
        generation: generation.id,
        qdrant_collection: names.collection.clone(),
        alias: names.alias.clone(),
        points,
        embedding_profile: generation.embedding_profile.clone(),
        sparse_profile: generation.sparse_profile.clone(),
        retired,
    }
}
