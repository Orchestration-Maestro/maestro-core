//! Publishing a chunk set as a generation: found again or created, built in
//! its own collection batch by batch, checked, and only then behind the
//! collection's alias.

use super::{
    batches::Target,
    dense::embedding_profile,
    error::{Error, Unverified},
    progress::{Progress, Report},
    projection::Projection,
    verify::{vectors, verify},
};
use crate::lexical;
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
    /// checked again and published; a published one is reported as it is.
    ///
    /// # Errors
    ///
    /// Before any work, [`Error::NotAnEmbedder`], [`Error::UnknownChunkSet`]
    /// and [`Error::Incomplete`]. Part way, [`Error::Embedding`],
    /// [`Error::Unreadable`], [`Error::Qdrant`] and [`Error::Stopped`], and
    /// the kernel's failures, all of which leave the generation as it was, so
    /// a rerun resumes it; and [`Error::Unverified`] when its collection
    /// fails a check, which fails the generation: the alias stays where it
    /// was.
    pub async fn publish_observed(
        &self,
        chunk_set: &str,
        resume: Option<&Progress>,
        observer: &mut impl FnMut(&Progress) -> ControlFlow<()>,
    ) -> Result<Report, Error> {
        let dimensions = self.dimensions()?;
        let set = self.complete(chunk_set)?;
        let (generation, step) = self.generation(&set)?;
        let names = Names::of(&set.collection_id, generation.id);
        if step == Step::Done {
            let points = generation.point_count.unwrap_or_default();
            return Ok(report(&set, &generation, &names, points, None));
        }
        let chunks = self
            .database
            .chunks(self.scopes, &set.id)
            .map_err(Error::ChunkSet)?;
        if step == Step::Build {
            self.ensure(&names, dimensions, generation.id).await?;
            let start = resume
                .filter(|progress| progress.generation == generation.id)
                .map_or(0, |progress| progress.indexed);
            let target = Target {
                collection: &names.collection,
                generation: generation.id,
                chunks: &chunks,
            };
            self.index(&target, start, observer).await?;
        }
        let points = self
            .check(&names, dimensions, &chunks, generation.id)
            .await?;
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
        let found = self
            .database
            .generations(self.scopes, &set.collection_id)
            .map_err(Error::Generation)?
            .into_iter()
            .rev()
            .filter(|generation| {
                generation.chunk_set_id == set.id
                    && generation.embedding_profile == embedding
                    && generation.sparse_profile == lexical::PROFILE
            })
            .find_map(|generation| Some((Step::of(generation.state)?, generation)));
        if let Some((step, generation)) = found {
            return Ok((generation, step));
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
    async fn ensure(&self, names: &Names, dimensions: u64, generation: i64) -> Result<(), Error> {
        let collection = &names.collection;
        if !self
            .qdrant
            .exists(collection)
            .await
            .map_err(Error::Qdrant)?
        {
            return self
                .qdrant
                .create(collection, dimensions)
                .await
                .map_err(Error::Qdrant);
        }
        let parameters = self
            .qdrant
            .parameters(collection)
            .await
            .map_err(Error::Qdrant)?;
        match vectors(&parameters, dimensions) {
            Ok(()) => Ok(()),
            Err(reason) => self.fail(generation, reason),
        }
    }

    /// The points the collection of `names` holds once it passes every
    /// check of a generation whose embedder gives `dimensions` and whose
    /// chunk set holds `chunks`; a check it fails fails the generation.
    async fn check(
        &self,
        names: &Names,
        dimensions: u64,
        chunks: &[Chunk],
        generation: i64,
    ) -> Result<u64, Error> {
        match verify(self.qdrant, &names.collection, dimensions, chunks)
            .await
            .map_err(Error::Qdrant)?
        {
            Ok(points) => Ok(points),
            Err(reason) => self.fail(generation, reason),
        }
    }

    /// Fails the generation `generation` for `reason`, and refuses its
    /// publication.
    fn fail<T>(&self, generation: i64, reason: Unverified) -> Result<T, Error> {
        self.database
            .fail_generation(generation)
            .map_err(Error::Generation)?;
        Err(Error::Unverified { generation, reason })
    }
}

/// The names of a generation in Qdrant.
#[derive(Debug)]
struct Names {
    /// Its collection, `maestro-<collection>-g<n>`.
    collection: String,
    /// Its collection's alias, `maestro-<collection>`.
    alias: String,
}

impl Names {
    /// The names of the generation `generation` of the collection
    /// `collection`.
    fn of(collection: &str, generation: i64) -> Self {
        Self {
            collection: format!("maestro-{collection}-g{generation}"),
            alias: format!("maestro-{collection}"),
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
