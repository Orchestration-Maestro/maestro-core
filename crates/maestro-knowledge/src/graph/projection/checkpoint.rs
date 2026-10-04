//! Immutable loader journal, held separately from native sidecars.
#![cfg_attr(
    not(feature = "engine"),
    allow(
        dead_code,
        reason = "default tests cover the persisted loader contract"
    )
)]
use super::{
    build::ProjectionBuild,
    import::ProjectionSnapshot,
    port::{EdgeFamily, InputMismatchKind, ProjectionError},
    writer::BuildVerification,
};
use maestro_filesystem::{Directory, EntryKind, PublicationChecks};
use maestro_kernel::{artifact::Digest, facts::PROJECTION_REBUILD_REPAIR};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::mem;
use std::{cell::RefCell, collections::BTreeMap, fmt::Display, io::Write as _, path::Path};

/// Same chunk size as index/batches.rs (BATCH); sets resume granularity, not a setting.
/// Pi has no graph-batch equivalent.
pub(super) const BATCH_ROWS: usize = 64;
/// Versioned immutable record format.
const VERSION: &str = "graph-loader/2";

/// Count is derived, never configured; the fixed-width ordinal cannot overflow.
pub(super) fn batch_count(rows: usize) -> Result<u32, ProjectionError> {
    u32::try_from(rows.div_ceil(BATCH_ROWS)).map_err(refusal)
}

/// One immutable ownership and input record, before any loader batch.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    /// Persisted schema.
    schema: String,
    /// Exact reserved build, immutable predecessor and native stamp format.
    build_id: i64,
    /// Head identity against which the build was reserved.
    expected_active_build_id: Option<i64>,
    /// Native stamp version expected by this loader.
    native_schema: String,
    /// Owning kernel job; lease holder/number may change after fenced takeover.
    job: String,
    /// Exact collection/generation.
    scope: (String, i64),
    /// Selected membership digest.
    claim_set: Digest,
    /// Resolution, version, settings identity and frozen lock.
    pins: [String; 4],
    /// Both families' row counts.
    counts: (usize, usize),
    /// Complete frozen row identity, including frozen review states.
    digest: Digest,
}

impl Manifest {
    /// Bind one frozen input to the already admitted producer.
    pub(super) fn expected(
        build: &ProjectionBuild,
        snapshot: &ProjectionSnapshot,
        predecessor: Option<i64>,
    ) -> Result<Self, ProjectionError> {
        if snapshot.scope != build.scope {
            return Err(ProjectionError::InputMismatch(
                InputMismatchKind::Resolution,
            ));
        }
        if snapshot.claim_set.id != build.claim_set_id {
            return Err(ProjectionError::InputMismatch(
                InputMismatchKind::Resolution,
            ));
        }
        if snapshot.resolution_id != build.resolution_id {
            return Err(ProjectionError::InputMismatch(
                InputMismatchKind::Resolution,
            ));
        }
        if snapshot.resolver_version != build.resolver_version {
            return Err(ProjectionError::InputMismatch(
                InputMismatchKind::Resolution,
            ));
        }
        batch_count(
            snapshot
                .edges
                .len()
                .checked_add(snapshot.facts.len())
                .ok_or_else(|| refusal("row count overflow"))?,
        )?;
        let expected = BuildVerification::expected(&snapshot.edges, &snapshot.facts)?;
        Ok(Self {
            schema: VERSION.into(),
            build_id: build.build_id,
            expected_active_build_id: predecessor,
            native_schema: super::schema::SCHEMA_VERSION.into(),
            job: build.lease.job.to_string(),
            scope: (build.scope.collection_id.clone(), build.scope.generation_id),
            claim_set: build.claim_set_id.clone(),
            pins: pins(build),
            counts: (snapshot.edges.len(), snapshot.facts.len()),
            digest: expected.content_digest,
        })
    }

    /// Validate ownership and every durable input before constructing a fresh backend.
    pub(super) fn validate(
        &self,
        build: &ProjectionBuild,
        predecessor: Option<i64>,
    ) -> Result<(), ProjectionError> {
        if self.schema != VERSION
            || self.build_id != build.build_id
            || self.expected_active_build_id != predecessor
            || self.native_schema != super::schema::SCHEMA_VERSION
            || self.job != build.lease.job.to_string()
            || self.scope != (build.scope.collection_id.clone(), build.scope.generation_id)
            || self.claim_set != build.claim_set_id
            || self.pins != pins(build)
        {
            return Err(refusal("manifest ownership or pins differ"));
        }
        self.count()?;
        Ok(())
    }

    /// Loader publication cannot substitute manually written rows or a partial prefix.
    pub(super) fn verify_final(&self, verified: &BuildVerification) -> Result<(), ProjectionError> {
        if verified
            .family_counts
            .get(&EdgeFamily::KnowledgeClaim)
            .copied()
            .unwrap_or(0)
            != self.counts.0
            || verified
                .family_counts
                .get(&EdgeFamily::CatalogDependency)
                .copied()
                .unwrap_or(0)
                != 0
            || verified.fact_count != self.counts.1
            || verified.content_digest != self.digest
        {
            return Err(refusal(
                "publication content differs from frozen loader manifest",
            ));
        }
        Ok(())
    }

    /// Bound listing and ordinals by the recorded row count.
    pub(super) fn count(&self) -> Result<u32, ProjectionError> {
        batch_count(
            self.counts
                .0
                .checked_add(self.counts.1)
                .ok_or_else(|| refusal("manifest count overflow"))?,
        )
    }
}

/// Durable verification of one ordinal, with explicit application IDs and cumulative digest.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    /// Persisted schema.
    schema: String,
    /// One-based fixed-width ordinal.
    ordinal: u32,
    /// IDs of this batch, in frozen loader order (edges then facts).
    ids: Vec<Digest>,
    /// Cumulative durable counts and content digest.
    verified: (usize, usize, Digest),
}

/// An identity-checked directory retained across every record operation.
#[derive(Debug)]
pub(super) struct Journal {
    /// Held loader directory, separate from the native file/WAL.
    directory: Directory,
    /// Written once; never replaced.
    pub(super) manifest: Manifest,
    /// Independently derived canonical records retained after load validation.
    expected: RefCell<BTreeMap<u32, Vec<u8>>>,
    /// Fail after this many successful removals.
    #[cfg(test)]
    pub(super) fail_cleanup_after: Option<usize>,
    /// One-shot post-install removal refusal for the native contract test.
    #[cfg(test)]
    pub(super) fail_cleanup: bool,
    /// Inject a disk-full refusal after durable native verification, before a record.
    #[cfg(test)]
    pub(super) fail_record: bool,
}

impl Journal {
    /// Reserve the loader record directory and publish its manifest create-new.
    pub(super) fn create(staging: &Path, manifest: Manifest) -> Result<Self, ProjectionError> {
        let directory = Directory::open_canonical(staging)
            .map_err(refusal)?
            .create_child("loader")
            .map_err(refusal)?;
        let journal = Self {
            directory,
            manifest,
            expected: RefCell::new(BTreeMap::new()),
            #[cfg(test)]
            fail_cleanup_after: None,
            #[cfg(test)]
            fail_cleanup: false,
            #[cfg(test)]
            fail_record: false,
        };
        journal.append(
            "manifest.json",
            &serde_json::to_vec(&journal.manifest).map_err(refusal)?,
        )?;
        Ok(journal)
    }

    /// Reopen only a loader-owned build; bound bytes by the admitted database-size ceiling.
    pub(super) fn open(
        staging: &Path,
        build: &ProjectionBuild,
        predecessor: Option<i64>,
        max_bytes: u64,
    ) -> Result<Self, ProjectionError> {
        let directory = Directory::open_canonical(staging)
            .map_err(refusal)?
            .child("loader")
            .map_err(refusal)?;
        let bytes = directory
            .read_regular_bounded("manifest.json", max_bytes)
            .map_err(refusal)?;
        let manifest: Manifest = serde_json::from_slice(&bytes).map_err(refusal)?;
        manifest.validate(build, predecessor)?;
        let journal = Self {
            directory,
            manifest,
            expected: RefCell::new(BTreeMap::new()),
            #[cfg(test)]
            fail_cleanup_after: None,
            #[cfg(test)]
            fail_cleanup: false,
            #[cfg(test)]
            fail_record: false,
        };
        journal.ordinals()?;
        Ok(journal)
    }

    /// Refuse gaps, extra/unknown files, directories and noncanonical ordinal names.
    pub(super) fn ordinals(&self) -> Result<u32, ProjectionError> {
        let count = self.manifest.count()?;
        let limit = usize::try_from(count)
            .map_err(refusal)?
            .checked_add(1)
            .ok_or_else(|| refusal("checkpoint count overflow"))?;
        let mut entries = self.directory.list_bounded(limit).map_err(refusal)?;
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        if entries.iter().any(|entry| entry.kind != EntryKind::File)
            || !entries.iter().any(|entry| entry.name == "manifest.json")
        {
            return Err(refusal("unknown loader entry or missing manifest"));
        }
        let mut ordinal = 0u32;
        for entry in entries
            .into_iter()
            .filter(|entry| entry.name != "manifest.json")
        {
            ordinal = ordinal
                .checked_add(1)
                .ok_or_else(|| refusal("ordinal overflow"))?;
            if ordinal > count || entry.name != name(ordinal).as_str() {
                return Err(refusal("checkpoint gap, order or unknown file"));
            }
        }
        Ok(ordinal)
    }

    /// Retain every authoritative expectation before any loader completion writes.
    pub(super) fn expect(&self, snapshot: &ProjectionSnapshot) -> Result<(), ProjectionError> {
        let mut expected = BTreeMap::new();
        for ordinal in 1..=self.manifest.count()? {
            expected.insert(
                ordinal,
                serde_json::to_vec(&checkpoint(snapshot, ordinal)?).map_err(refusal)?,
            );
        }
        *self.expected.borrow_mut() = expected;
        Ok(())
    }

    /// Compare every immutable checkpoint with its independently derived frozen content.
    pub(super) fn validate_checkpoint(
        &self,
        snapshot: &ProjectionSnapshot,
        ordinal: u32,
    ) -> Result<(), ProjectionError> {
        let expected = serde_json::to_vec(&checkpoint(snapshot, ordinal)?).map_err(refusal)?;
        let bytes = self
            .directory
            .read_regular_bounded(&name(ordinal), expected.len() as u64)
            .map_err(refusal)?;
        if bytes != expected {
            return Err(refusal("checkpoint IDs, digest or format differ"));
        }
        self.expected.borrow_mut().insert(ordinal, expected);
        Ok(())
    }

    /// Only called after the reopened native verification equals the frozen prefix.
    pub(super) fn record(
        &self,
        snapshot: &ProjectionSnapshot,
        ordinal: u32,
    ) -> Result<(), ProjectionError> {
        #[cfg(test)]
        if self.fail_record {
            return Err(refusal("injected disk full while creating checkpoint"));
        }
        let expected = serde_json::to_vec(&checkpoint(snapshot, ordinal)?).map_err(refusal)?;
        self.append(&name(ordinal), &expected)?;
        self.expected.borrow_mut().insert(ordinal, expected);
        Ok(())
    }

    /// Recheck held expectations before installation, never adopt newly read bytes.
    pub(super) fn revalidate(&self) -> Result<(), ProjectionError> {
        self.compare(
            "manifest.json",
            &serde_json::to_vec(&self.manifest).map_err(refusal)?,
        )?;
        let expected = self.expected.borrow();
        self.ordinals()?;
        if expected.len() != self.manifest.count()? as usize {
            return Err(refusal("checkpoint expectations incomplete"));
        }
        for (ordinal, bytes) in expected.iter() {
            self.compare(&name(*ordinal), bytes)?;
        }
        Ok(())
    }

    /// Compare bounded bytes against authoritative canonical content.
    fn compare(&self, name: &str, expected: &[u8]) -> Result<(), ProjectionError> {
        let bytes = self
            .directory
            .read_regular_bounded(name, expected.len() as u64)
            .map_err(refusal)?;
        if bytes != expected {
            return Err(refusal("loader record changed"));
        }
        Ok(())
    }

    /// Only after native install and readiness: remove known records, manifest last.
    pub(super) fn cleanup(&mut self, staging: &Path) -> Result<(), ProjectionError> {
        let completed = self.ordinals()?;
        for ordinal in (1..=completed).rev() {
            let bytes = self
                .expected
                .borrow()
                .get(&ordinal)
                .cloned()
                .ok_or_else(|| refusal("missing checkpoint expectation"))?;
            self.remove(&name(ordinal), &bytes)?;
        }
        self.remove(
            "manifest.json",
            &serde_json::to_vec(&self.manifest).map_err(refusal)?,
        )?;
        Directory::open_canonical(staging)
            .map_err(refusal)?
            .remove_created_child("loader", &self.directory)
            .map_err(refusal)
    }

    /// Removal failure remains observable even though the projection is already published.
    fn remove(&mut self, name: &str, bytes: &[u8]) -> Result<(), ProjectionError> {
        #[cfg(test)]
        if mem::take(&mut self.fail_cleanup) {
            return Err(refusal("injected checkpoint removal failure"));
        }
        #[cfg(test)]
        if let Some(remaining) = self.fail_cleanup_after.as_mut() {
            if *remaining == 0 {
                return Err(refusal("injected checkpoint removal failure"));
            }
            *remaining -= 1;
        }
        self.directory
            .remove_verified(name, bytes, None)
            .map_err(refusal)
    }

    /// Create, sync and publish without replacing any source or destination.
    fn append(&self, name: &str, bytes: &[u8]) -> Result<(), ProjectionError> {
        self.directory.verify_named().map_err(refusal)?;
        let temporary = format!("{name}.pending");
        let mut file = self.directory.create_new(&temporary).map_err(refusal)?;
        file.write_all(bytes).map_err(refusal)?;
        file.sync_all().map_err(refusal)?;
        self.directory.sync().map_err(refusal)?;
        self.directory
            .publish_verified(
                &temporary,
                name,
                bytes,
                PublicationChecks {
                    after_source_open: || self.directory.verify_named(),
                    before_link: || self.directory.verify_named(),
                    after_link: || self.directory.verify_named(),
                },
            )
            .map_err(refusal)?;
        self.directory.sync().map_err(refusal)?;
        self.directory
            .remove_created_bytes(&temporary, &file, bytes)
            .map_err(refusal)?;
        self.directory.sync().map_err(refusal)
    }
}

/// Stable one-based u32 spelling; lexical order equals ordinal order.
fn name(ordinal: u32) -> String {
    format!("{ordinal:010}.json")
}

/// Shared prefix boundary for writes and independent durable expectations.
pub(super) fn bounds(
    snapshot: &ProjectionSnapshot,
    ordinal: u32,
) -> Result<(usize, usize), ProjectionError> {
    let rows = snapshot
        .edges
        .len()
        .checked_add(snapshot.facts.len())
        .ok_or_else(|| refusal("row count overflow"))?;
    if ordinal > batch_count(rows)? {
        return Err(refusal("ordinal exceeds frozen batch count"));
    }
    let end = usize::try_from(ordinal)
        .map_err(refusal)?
        .checked_mul(BATCH_ROWS)
        .ok_or_else(|| refusal("ordinal size overflow"))?
        .min(rows);
    Ok((
        end.min(snapshot.edges.len()),
        end.saturating_sub(snapshot.edges.len()),
    ))
}

/// Expectation uses the same frozen encoder as publication, not a second row encoding.
pub(super) fn prefix(
    snapshot: &ProjectionSnapshot,
    ordinal: u32,
) -> Result<BuildVerification, ProjectionError> {
    let (edges, facts) = bounds(snapshot, ordinal)?;
    BuildVerification::expected(
        snapshot
            .edges
            .get(..edges)
            .ok_or_else(|| refusal("edge prefix outside snapshot"))?,
        snapshot
            .facts
            .get(..facts)
            .ok_or_else(|| refusal("fact prefix outside snapshot"))?,
    )
}

/// Derive one immutable record from authoritative inputs, after durable verification.
fn checkpoint(snapshot: &ProjectionSnapshot, ordinal: u32) -> Result<Checkpoint, ProjectionError> {
    let previous = ordinal
        .checked_sub(1)
        .ok_or_else(|| refusal("zero checkpoint ordinal"))?;
    let (start_edges, start_facts) = bounds(snapshot, previous)?;
    let (edges, facts) = bounds(snapshot, ordinal)?;
    let verified = prefix(snapshot, ordinal)?;
    Ok(Checkpoint {
        schema: VERSION.into(),
        ordinal,
        ids: snapshot
            .edges
            .get(start_edges..edges)
            .ok_or_else(|| refusal("edge batch outside snapshot"))?
            .iter()
            .map(|edge| edge.id.clone())
            .chain(
                snapshot
                    .facts
                    .get(start_facts..facts)
                    .ok_or_else(|| refusal("fact batch outside snapshot"))?
                    .iter()
                    .map(|fact| fact.claim.id.clone()),
            )
            .collect(),
        verified: (edges, facts, verified.content_digest),
    })
}

/// Persisted spelling agrees with the native/G28b pins.
fn pins(build: &ProjectionBuild) -> [String; 4] {
    [
        build.resolution_id.as_str().into(),
        build.resolver_version.clone(),
        build.settings_identity.as_str().into(),
        build.frozen_lock.as_str().into(),
    ]
}

/// Every recovery refusal names the same runnable rebuild remedy.
pub(super) fn refusal(error: impl Display) -> ProjectionError {
    ProjectionError::Backend(format!(
        "loader resume refused: {error}; {PROJECTION_REBUILD_REPAIR}"
    ))
}
