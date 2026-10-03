//! One bounded loader over the existing lease-bound producer and writer.
use crate::graph::projection::adapter::Session;
use crate::graph::projection::{
    ProjectionError, ProjectionSnapshot,
    checkpoint::{self, Journal, Manifest},
};

impl Session<'_> {
    /// Validate immutable ownership, checkpoints and uncertain native content before writing.
    pub(in crate::graph::projection) fn load(
        &mut self,
        snapshot: &ProjectionSnapshot,
    ) -> Result<(), ProjectionError> {
        let expected_manifest = Manifest::expected(&self.build, snapshot)?;
        // Verification checks the live lease before creating even the manifest.
        let durable = self.verify()?;
        if self.journal.is_none() {
            if durable != checkpoint::prefix(snapshot, 0)? {
                return Err(checkpoint::refusal(
                    "unrelated native rows before loader start",
                ));
            }
            self.journal = Some(Journal::create(&self.staging_path()?, expected_manifest)?);
        } else if self.journal.as_ref().map(|journal| &journal.manifest) != Some(&expected_manifest)
        {
            return Err(checkpoint::refusal("frozen snapshot changed"));
        }
        let journal = self
            .journal
            .as_ref()
            .ok_or_else(|| checkpoint::refusal("missing manifest"))?;
        let count = journal.manifest.count()?;
        let mut completed = journal.ordinals()?;
        for ordinal in 1..=completed {
            journal.validate_checkpoint(snapshot, ordinal)?;
        }
        if durable != checkpoint::prefix(snapshot, completed)? {
            // Native transactions are atomic: only the immediate next complete batch is uncertain.
            if completed == count || durable != checkpoint::prefix(snapshot, completed + 1)? {
                return Err(checkpoint::refusal(
                    "durable rows differ from checkpoint or uncertain batch",
                ));
            }
            completed += 1;
            journal.record(snapshot, completed)?;
        }
        self.loader_validated = true;
        for ordinal in completed..count {
            let ordinal = ordinal + 1;
            let (start_edges, start_facts) = checkpoint::bounds(snapshot, ordinal - 1)?;
            let (edges, facts) = checkpoint::bounds(snapshot, ordinal)?;
            self.write_batch(
                snapshot
                    .edges
                    .get(start_edges..edges)
                    .ok_or_else(|| checkpoint::refusal("edge batch outside snapshot"))?,
                snapshot
                    .facts
                    .get(start_facts..facts)
                    .ok_or_else(|| checkpoint::refusal("fact batch outside snapshot"))?,
                None,
            )?;
            if self.verify()? != checkpoint::prefix(snapshot, ordinal)? {
                return Err(checkpoint::refusal("batch durable verification differs"));
            }
            self.journal
                .as_ref()
                .ok_or_else(|| checkpoint::refusal("missing manifest"))?
                .record(snapshot, ordinal)?;
        }
        Ok(())
    }
}
