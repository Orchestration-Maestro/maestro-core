//! Digest-bound replacement plans over the existing C04 state writer.
use super::{
    effects,
    plan::{FilePlan, digest, validate_id, validate_relative_path},
    publication::write_new,
    recovery::record_bytes,
    transition::ReplacementPlan,
};
use crate::{
    limits::Limits,
    policy::workspace::{Access, CheckedTrust},
};
use std::{
    io::{self, Read as _},
    path::Path,
    str,
};

impl FilePlan {
    /// Preview one shared file's exact old-to-new transition, writing nothing.
    ///
    /// # Errors
    /// Refuses unsafe paths, oversized files, links and failed reads.
    pub fn preview_replacement(
        root: &Path,
        path: &str,
        new: Vec<u8>,
        trust: &CheckedTrust<'_>,
    ) -> io::Result<Self> {
        Self::preview_edit(root, path, |_| Ok(new), trust)
    }

    /// Capture once, then compute a shared-file edit from exactly those bytes.
    /// Entry-level ownership belongs to the adapter; C04 binds unchanged-file preconditions.
    ///
    /// # Errors
    /// Refuses unsafe paths, oversized files, links and adapter refusals.
    pub fn preview_edit(
        root: &Path,
        path: &str,
        edit: impl FnOnce(Option<&[u8]>) -> io::Result<Vec<u8>>,
        trust: &CheckedTrust<'_>,
    ) -> io::Result<Self> {
        let replacement = ReplacementPlan::preview_edit(root, path, edit, trust)?;
        Ok(Self {
            id: replacement.id.clone(),
            entries: Vec::new(),
            applied: false,
            replacement: Some(Box::new(replacement)),
        })
    }

    /// Whether a shared-file transition proposes different bytes.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.replacement.as_ref().is_some_and(|plan| plan.changed())
    }

    /// Preflight a shared transition before any other plan creates effects.
    ///
    /// # Errors
    /// Refuses non-replacement plans, drift and policy denials.
    pub fn check_replacement(&self, root: &Path, trust: &CheckedTrust<'_>) -> io::Result<()> {
        self.replacement
            .as_ref()
            .ok_or_else(|| io::Error::other("not a replacement plan"))?
            .check(root, trust)
    }

    /// Resume a versioned shared-file transition after journal or rename interruption.
    ///
    /// # Errors
    /// Refuses malformed journals, changed bytes and policy denials.
    pub fn recover_replacement(root: &Path, id: &str, trust: &CheckedTrust<'_>) -> io::Result<()> {
        ReplacementPlan::recover(root, id, trust)
    }
}

impl ReplacementPlan {
    /// Read once and let a host compute new bytes from that exact validated snapshot.
    ///
    /// # Errors
    /// Refuses unsafe paths, oversized files, links, failed reads and adapter refusals.
    fn preview_edit(
        root: &Path,
        path: &str,
        edit: impl FnOnce(Option<&[u8]>) -> io::Result<Vec<u8>>,
        trust: &CheckedTrust<'_>,
    ) -> io::Result<Self> {
        validate_relative_path(path)?;
        let old = read_snapshot(root, path, trust)?;
        let new = edit(old.as_deref())?;
        let mut plan = Self {
            schema: "maestro-replacement/1".to_owned(),
            path: path.to_owned(),
            old,
            new,
            id: String::new(),
        };
        plan.id = plan.identity()?;
        plan.validate()?;
        Ok(plan)
    }

    /// Whether this snapshot actually proposes a byte change.
    #[must_use]
    fn changed(&self) -> bool {
        self.old.as_deref() != Some(self.new.as_slice())
    }

    /// Recheck the captured file and fresh write policy before another plan creates effects.
    ///
    /// # Errors
    /// Refuses outside-trust writes and any changed preview, including newly appeared targets.
    fn check(&self, root: &Path, trust: &CheckedTrust<'_>) -> io::Result<()> {
        self.validate()?;
        effects::check(root, &self.path, trust)?;
        if read_snapshot(root, &self.path, trust)? != self.old {
            return Err(io::Error::other(format!(
                "{} changed since preview; run preview again",
                self.path
            )));
        }
        Ok(())
    }

    /// Recover one explicit transition, validating the journal before permitting effects.
    ///
    /// # Errors
    /// Refuses malformed journals, drift and outside-trust writes.
    fn recover(root: &Path, id: &str, trust: &CheckedTrust<'_>) -> io::Result<()> {
        validate_id(id)?;
        let path = journal_path(id);
        let bytes = read_bounded(root, &path, Limits::PRODUCTION.archive_entry_bytes, trust)?
            .ok_or_else(|| {
                io::Error::other("no replacement journal; inspect the target and run preview again")
            })?;
        let plan: Self = toml::from_str(str::from_utf8(&bytes).map_err(io::Error::other)?)
            .map_err(io::Error::other)?;
        plan.validate()?;
        if plan.id != id {
            return Err(io::Error::other("replacement journal identity mismatch"));
        }
        plan.apply_with_failure(root, trust, None)
    }

    /// Private interruption seam before and after the atomic publication.
    pub(crate) fn apply_with_failure(
        &self,
        root: &Path,
        trust: &CheckedTrust<'_>,
        fail: Option<usize>,
    ) -> io::Result<()> {
        self.validate()?;
        effects::check(root, &self.path, trust)?;
        let journal = journal_path(&self.id);
        effects::check(root, &journal, trust)?;
        let encoded = record_bytes(self)?;
        let pending = read_bounded(
            root,
            &journal,
            Limits::PRODUCTION.archive_entry_bytes,
            trust,
        )?;
        if pending.as_ref().is_some_and(|bytes| bytes != &encoded) {
            return Err(io::Error::other("conflicting replacement journal"));
        }
        let current = read_snapshot(root, &self.path, trust)?;
        if pending.is_some() && current.as_deref() == Some(self.new.as_slice()) {
            effects::remove(root, &journal, &encoded, None, trust)?;
            return Ok(());
        }
        if current != self.old {
            return Err(io::Error::other(format!(
                "{} changed since preview; run preview again",
                self.path
            )));
        }
        if !self.changed() {
            return Ok(());
        }
        if pending.is_none() {
            write_new(root, (&journal, &encoded), false, trust)?;
        }
        interrupt(fail, 0)?;
        match &self.old {
            Some(bytes) => trust
                .authorize(root, Path::new(&self.path), Access::Write)?
                .replace_verified(bytes, &self.new)?,
            None => write_new(root, (&self.path, &self.new), false, trust)?,
        }
        interrupt(fail, 1)?;
        effects::remove(root, &journal, &encoded, None, trust)
    }

    /// Bind schema/path and byte snapshots, with length framing supplied by TOML.
    fn identity(&self) -> io::Result<String> {
        let mut unsigned = self.clone();
        unsigned.id.clear();
        record_bytes(&unsigned).map(|bytes| digest(&bytes))
    }

    /// Never permit recovered state to broaden paths or misbind transition bytes.
    fn validate(&self) -> io::Result<()> {
        validate_relative_path(&self.path)?;
        validate_id(&self.id)?;
        if self.schema != "maestro-replacement/1" || self.identity()? != self.id {
            return Err(io::Error::other("invalid replacement journal"));
        }
        let limit = Limits::PRODUCTION.source_file_bytes;
        if self.new.len() as u64 > limit
            || self
                .old
                .as_ref()
                .is_some_and(|old| old.len() as u64 > limit)
        {
            return Err(io::Error::other(
                "replacement exceeds source file byte limit",
            ));
        }
        Ok(())
    }
}

/// Bound shared input bytes by the existing catalog source ceiling before decoding or allocation.
fn read_snapshot(root: &Path, path: &str, trust: &CheckedTrust<'_>) -> io::Result<Option<Vec<u8>>> {
    read_bounded(root, path, Limits::PRODUCTION.source_file_bytes, trust)
}

/// Metadata uses the existing archive-entry ceiling; file snapshots use the source ceiling.
fn read_bounded(
    root: &Path,
    path: &str,
    limit: u64,
    trust: &CheckedTrust<'_>,
) -> io::Result<Option<Vec<u8>>> {
    let result = trust
        .authorize(root, Path::new(path), Access::Read)
        .and_then(|lease| lease.open_read());
    let file = match result {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::other(format!(
            "{path} exceeds the {limit}-byte limit"
        )));
    }
    Ok(Some(bytes))
}

/// C04 state names are private to the writer, never host-owned output paths.
fn journal_path(id: &str) -> String {
    format!(
        ".maestro-files/replace-{}.toml",
        id.strip_prefix("sha256:").unwrap_or(id)
    )
}

/// Simulate a process stop at a durable journal/publication boundary.
fn interrupt(fail: Option<usize>, point: usize) -> io::Result<()> {
    if fail == Some(point) {
        return Err(io::Error::other("injected replacement interruption"));
    }
    Ok(())
}
