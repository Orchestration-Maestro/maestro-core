//! Loads and caches authoritative source records for one assembly request.

use super::{
    families::{SectionOccurrence, section_occurrences},
    sections::SectionIndex,
    types::EvidenceError,
};
use maestro_canonicalization::{
    CanonicalDocument, Severity, SourceSpan, ValidationStatus, validate_document,
};
use maestro_kernel::{
    document::{Disposition, Document, Outcome, Revision, RevisionStatus},
    evidence::Span,
    generation::Generation,
    retrieval::ReadControl,
    scope::ScopeSet,
    store::Database,
};
use std::{collections::BTreeMap, sync::atomic::Ordering, time::Instant};

/// One revision's authorized records and verified original source.
pub(crate) struct EvidenceSource {
    /// The immutable revision and its artifact identities.
    pub(crate) revision: Revision,
    /// The source document admitted by the read scopes.
    pub(crate) document: Document,
    /// The accepted disposition and its explicit source limitations.
    pub(crate) disposition: Disposition,
    /// The canonical representation replayed against the original bytes.
    pub(crate) canonical: CanonicalDocument,
    /// The exact original UTF-8 Markdown.
    pub(crate) markdown: String,
    /// Validated source ranges and lexical section relationships.
    pub(crate) sections: SectionIndex,
    /// One-pass section path occurrences for family matching.
    pub(crate) section_occurrences: BTreeMap<String, SectionOccurrence>,
}

/// Request-local cache of source artifacts, keyed by their authorized revision.
/// ponytail: retain full source per request; cap bytes only if measurements show memory pressure.
pub(crate) struct SourceCache<'a> {
    /// Kernel store used for reads authorized by the admitted scopes.
    database: &'a Database,
    /// Immutable read-grant snapshot accepted with the request.
    scopes: &'a ScopeSet,
    /// Shared cancellation flag and inherited deadline.
    control: &'a ReadControl,
    /// Loaded source records keyed by authorized revision ID.
    sources: BTreeMap<String, EvidenceSource>,
    #[cfg(test)]
    load_counts: BTreeMap<String, ArtifactLoadCounts>,
}

#[cfg(test)]
#[derive(Default)]
struct ArtifactLoadCounts {
    canonical: usize,
    original: usize,
}

impl<'a> SourceCache<'a> {
    /// Starts an empty request-local cache over one admitted permission snapshot.
    pub(crate) fn new(
        database: &'a Database,
        scopes: &'a ScopeSet,
        control: &'a ReadControl,
    ) -> Self {
        Self {
            database,
            scopes,
            control,
            sources: BTreeMap::new(),
            #[cfg(test)]
            load_counts: BTreeMap::new(),
        }
    }

    /// Returns one verified source, loading both immutable artifacts at most once.
    pub(crate) fn load(
        &mut self,
        revision_id: &str,
        generation: &Generation,
    ) -> Result<&EvidenceSource, EvidenceError> {
        self.check()?;
        if !self.sources.contains_key(revision_id) {
            let source = self.load_source(revision_id, &generation.collection_id)?;
            self.sources.insert(revision_id.to_owned(), source);
        }
        let source = self
            .sources
            .get(revision_id)
            .ok_or(EvidenceError::WorkerFailed)?;
        self.check()?;
        Ok(source)
    }

    /// Returns an already-loaded authoritative revision without mutating the cache.
    pub(crate) fn get(&self, revision_id: &str) -> Option<&EvidenceSource> {
        self.sources.get(revision_id)
    }

    /// Rejects empty, reversed, out-of-range and non-boundary chunk spans.
    pub(crate) fn validate_chunk_span(
        source: &EvidenceSource,
        span: Span,
    ) -> Result<(), EvidenceError> {
        let source_span = SourceSpan {
            start: span.start,
            end: span.end,
        };
        if span.start >= span.end || !source_span.is_valid(&source.markdown) {
            return Err(integrity("candidate chunk has an invalid source span"));
        }
        Ok(())
    }

    /// Counts actual artifact reads in tests without adding a store abstraction.
    #[cfg(test)]
    pub(crate) fn load_counts(&self, revision_id: &str) -> Option<(usize, usize)> {
        self.load_counts
            .get(revision_id)
            .map(|counts| (counts.canonical, counts.original))
    }

    /// Loads and validates one revision's scoped records and exact artifacts.
    fn load_source(
        &mut self,
        revision_id: &str,
        collection_id: &str,
    ) -> Result<EvidenceSource, EvidenceError> {
        self.check()?;
        let revision = self
            .database
            .revision(self.scopes, revision_id)
            .map_err(EvidenceError::Records)?
            .ok_or(EvidenceError::NotVisible)?;
        self.check()?;
        let document = self
            .database
            .document(self.scopes, &revision.document_id)
            .map_err(EvidenceError::Records)?
            .ok_or(EvidenceError::NotVisible)?;
        self.check()?;
        let disposition = self
            .database
            .disposition(self.scopes, revision_id)
            .map_err(EvidenceError::Records)?
            .ok_or(EvidenceError::NotVisible)?;
        self.check()?;

        if revision.document_id != document.id || document.collection_id != collection_id {
            return Err(integrity("revision and document identity do not match"));
        }
        if revision.status == RevisionStatus::Failed
            || !matches!(
                disposition.outcome,
                Outcome::Accepted | Outcome::AcceptedWithWarnings
            )
        {
            return Err(integrity("evidence eligibility changed during assembly"));
        }

        self.check()?;
        #[cfg(test)]
        {
            self.load_counts
                .entry(revision_id.to_owned())
                .or_default()
                .canonical += 1;
        }
        let canonical_bytes = self
            .database
            .get(&revision.canonical_digest)
            .map_err(EvidenceError::Store)?;
        self.check()?;
        let canonical: CanonicalDocument = serde_json::from_slice(&canonical_bytes)
            .map_err(|_| integrity("canonical artifact is not valid JSON"))?;
        if canonical.document_id != document.id
            || canonical.revision_id != revision.id
            || canonical.content_hash != format!("sha256:{}", revision.original_digest.as_str())
            || canonical.original_markdown_reference.content_hash != canonical.content_hash
            || canonical.validation_status == ValidationStatus::Failed
        {
            return Err(integrity(
                "canonical artifact identity or status is invalid",
            ));
        }

        self.check()?;
        #[cfg(test)]
        {
            self.load_counts
                .entry(revision_id.to_owned())
                .or_default()
                .original += 1;
        }
        let original_bytes = self
            .database
            .get(&revision.original_digest)
            .map_err(EvidenceError::Store)?;
        self.check()?;
        let markdown =
            String::from_utf8(original_bytes).map_err(|_| integrity("source is not UTF-8"))?;
        if canonical.original_markdown_reference.byte_length != markdown.len() {
            return Err(integrity(
                "canonical source length does not match its artifact",
            ));
        }
        if validate_document(&canonical, &markdown)
            .iter()
            .any(|finding| finding.severity == Severity::Error)
        {
            return Err(integrity("canonical document replay found an error"));
        }
        self.check()?;
        let sections = SectionIndex::new(&canonical, &markdown)
            .map_err(|_| integrity("canonical source spans or links are invalid"))?;
        let section_occurrences = section_occurrences(&canonical)
            .map_err(|_| integrity("canonical section identities are invalid"))?;
        self.check()?;
        Ok(EvidenceSource {
            revision,
            document,
            disposition,
            canonical,
            markdown,
            sections,
            section_occurrences,
        })
    }

    /// Stops before and after blocking work when cancellation or expiry fires.
    fn check(&self) -> Result<(), EvidenceError> {
        if self.control.cancelled.load(Ordering::Relaxed) || Instant::now() >= self.control.deadline
        {
            Err(EvidenceError::TimedOut)
        } else {
            Ok(())
        }
    }
}

/// Keeps integrity diagnostics independent of source text and record IDs.
fn integrity(reason: &str) -> EvidenceError {
    EvidenceError::Integrity(reason.to_owned())
}
