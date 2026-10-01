//! The documents a preparation leaves out: those whose latest revision, in
//! record order, the quality gate does not let through
//! ([`quality::eligible`](crate::quality::eligible)), each with that revision
//! and why. No older revision stands in for it.

use super::{
    exact::Kernel,
    failure::Error,
    report::{Ineligibility, LeftOut},
};
use maestro_kernel::document::{Outcome, Revision, RevisionStatus};
use std::collections::{BTreeMap, BTreeSet};

impl Kernel<'_> {
    /// The documents of the collection whose latest revision in record order
    /// is none of `eligible`, in document order, each with that revision and
    /// why it is not eligible.
    ///
    /// # Errors
    ///
    /// [`Error::Records`] when the kernel cannot be read.
    pub(super) fn left_out(&self, eligible: &[Revision]) -> Result<Vec<LeftOut>, Error> {
        let eligible: BTreeSet<&str> = eligible
            .iter()
            .map(|revision| revision.id.as_str())
            .collect();
        // Collected in record order, so each document keeps its last revision.
        let latest: BTreeMap<String, Revision> = self
            .database
            .revisions(self.scopes, self.collection)
            .map_err(Error::Records)?
            .into_iter()
            .map(|revision| (revision.document_id.clone(), revision))
            .collect();
        let mut left_out = Vec::new();
        for revision in latest.into_values() {
            if eligible.contains(revision.id.as_str()) {
                continue;
            }
            let reason = self.ineligibility(&revision)?;
            let document = self.document_of(&revision)?;
            left_out.push(LeftOut {
                document: document.id,
                source_ref: document.source_ref,
                revision: revision.id,
                reason,
            });
        }
        Ok(left_out)
    }

    /// Why `revision`, the latest of its document, is not eligible: failed
    /// before anything else, else what its disposition holds it back for, or
    /// undecided.
    fn ineligibility(&self, revision: &Revision) -> Result<Ineligibility, Error> {
        if revision.status == RevisionStatus::Failed {
            return Ok(Ineligibility::Failed);
        }
        let disposition = self
            .database
            .disposition(self.scopes, &revision.id)
            .map_err(Error::Records)?;
        Ok(match disposition.map(|disposition| disposition.outcome) {
            Some(Outcome::NeedsReextraction) => Ineligibility::NeedsReextraction,
            Some(Outcome::Quarantined) => Ineligibility::Quarantined,
            Some(Outcome::Excluded) => Ineligibility::Excluded,
            // Accepted after the eligible revisions were read, which a gate
            // running beside the preparation does: the next chunk set takes
            // it.
            Some(Outcome::Accepted | Outcome::AcceptedWithWarnings) | None => {
                Ineligibility::Undecided
            }
        })
    }
}
