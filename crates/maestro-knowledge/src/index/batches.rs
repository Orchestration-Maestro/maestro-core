//! Writing a generation's points, a batch at a time: each chunk's prepared
//! input represented by the embedder and the analyzer, with its payload, then
//! written to the generation's collection and shown to the caller, which
//! journals it.

use super::{
    dense::embed, error::Error, point::point, progress::Progress, projection::Projection,
    provenance::Provenance, sparse::Lengths,
};
use maestro_kernel::{chunk_set::Chunk, gateway::ModelPort};
use qdrant_client::qdrant::PointStruct;
use std::{ops::ControlFlow, time::Duration};

/// How many chunks one batch represents and writes.
const BATCH: usize = 64;

/// How long the embedder may take to answer one batch: long enough to load
/// into free room from cold, then embed it.
const DEADLINE: Duration = Duration::from_secs(60);

/// The generation whose points a build writes.
#[derive(Debug)]
pub(super) struct Target<'a> {
    /// Its collection in Qdrant.
    pub(super) collection: &'a str,
    /// Its id.
    pub(super) generation: i64,
    /// Every chunk of its chunk set, in record order.
    pub(super) chunks: &'a [Chunk],
}

impl<P: ModelPort> Projection<'_, P> {
    /// Writes the points of the chunks of `target` after its first `start`,
    /// in batches of [`BATCH`], and shows `observer` the progress once each
    /// batch is written. The sparse vectors are weighed against the average
    /// term count of every chunk of the set, which a first pass counts,
    /// unless no chunk is left to write.
    ///
    /// # Errors
    ///
    /// [`Error::Stopped`] when `observer` breaks, and the failures of
    /// [`Projection::publish_observed`] part way: what was written stays.
    pub(super) async fn index(
        &self,
        target: &Target<'_>,
        start: u64,
        observer: &mut impl FnMut(&Progress) -> ControlFlow<()>,
    ) -> Result<(), Error> {
        let left = usize::try_from(start)
            .ok()
            .and_then(|start| target.chunks.get(start..))
            .unwrap_or_default();
        if left.is_empty() {
            return Ok(());
        }
        let mut lengths = Lengths::default();
        for chunk in target.chunks {
            lengths.add(&self.input(chunk)?);
        }
        let mut indexed = start;
        for batch in left.chunks(BATCH) {
            let inputs = batch
                .iter()
                .map(|chunk| self.input(chunk))
                .collect::<Result<Vec<_>, _>>()?;
            let dense = embed(self.port, self.card, &inputs, DEADLINE)
                .await
                .map_err(|failure| Error::Embedding {
                    at: indexed,
                    failure,
                })?;
            let points = self.points(batch, &inputs, &dense, &lengths)?;
            self.qdrant
                .upsert(target.collection, points)
                .await
                .map_err(Error::Qdrant)?;
            indexed += u64::try_from(batch.len()).unwrap_or(u64::MAX);
            let progress = Progress {
                generation: target.generation,
                indexed,
                chunks: u64::try_from(target.chunks.len()).unwrap_or(u64::MAX),
                average_length: lengths.mean(),
            };
            if observer(&progress).is_break() {
                return Err(Error::Stopped);
            }
        }
        Ok(())
    }

    /// The prepared input of `chunk`, the text its chunk set counted, read
    /// from the artifact its digest names.
    fn input(&self, chunk: &Chunk) -> Result<String, Error> {
        let bytes = self.database.get(&chunk.digest).map_err(Error::Artifacts)?;
        String::from_utf8(bytes).map_err(|_| Error::Unreadable {
            chunk: chunk.id.clone(),
            reason: format!(
                "its prepared input sha256:{} is not UTF-8",
                chunk.digest.as_str()
            ),
        })
    }

    /// The points of `batch`, whose prepared inputs are `inputs` and whose
    /// dense vectors are `dense`, in its order, each sparse vector weighed
    /// against `lengths`. A chunk set records a revision's chunks together,
    /// so what the kernel says of a revision is read once for each run of its
    /// chunks.
    fn points(
        &self,
        batch: &[Chunk],
        inputs: &[String],
        dense: &[Vec<f32>],
        lengths: &Lengths,
    ) -> Result<Vec<PointStruct>, Error> {
        let mut points = Vec::with_capacity(batch.len());
        let mut represented = inputs.iter().zip(dense);
        for run in batch.chunk_by(|first, next| first.revision_id == next.revision_id) {
            let Some(first) = run.first() else {
                continue;
            };
            let provenance = Provenance::read(self.database, self.scopes, first)?;
            for (chunk, (text, vector)) in run.iter().zip(represented.by_ref()) {
                let sparse = lengths.vector(text);
                points.push(point(chunk, &provenance, vector, sparse.as_ref())?);
            }
        }
        Ok(points)
    }
}
