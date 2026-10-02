//! Apply exclusive file plans, commit ownership last, and recover proven states.
use super::{
    effects,
    names::{journal_name, ownership_name},
    plan::{FilePlan, digest, ownership_matches},
    recovery::{read_optional, record_bytes},
};
use crate::policy::workspace::{Access, CheckedTrust};
use serde::Serialize;
#[cfg(unix)]
use std::fs::Metadata;
use std::{
    io::{self, ErrorKind},
    path::Path,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Unique process-local suffixes for unpublished state-record files.
static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

/// A completed transaction's durable ownership record.
#[derive(Serialize)]
struct Ownership<'a> {
    /// The immutable plan identity.
    id: &'a str,
    /// Every path and content digest created by the transaction.
    files: Vec<OwnedFile<'a>>,
}

/// One file claimed by the transaction after exclusive creation.
#[derive(Serialize)]
struct OwnedFile<'a> {
    /// The root-relative name.
    path: &'a str,
    /// The bytes' SHA-256 digest.
    digest: &'a str,
    /// Stable Unix identity; absent on platforms without a stable std identity.
    #[serde(default)]
    identity: Option<FileIdentity>,
}

/// Stable filesystem identity recorded for a created target on Unix.
#[derive(Serialize)]
struct FileIdentity {
    /// Device number from the open file's metadata.
    device: u64,
    /// Inode number from the open file's metadata.
    inode: u64,
}

/// Apply a previewed plan, publishing ownership only after every exclusive create is durable.
///
/// A target that exists without a committed ownership record is refused, even when its bytes
/// match the plan. Recovery cannot prove who created a target if a crash occurred after its create
/// but before ownership was committed; that target is retained for an explicit user decision.
///
/// # Errors
/// Returns an error for an existing or changed target, journal conflict, or filesystem failure.
pub fn apply(root: &Path, plan: &FilePlan, trust: &CheckedTrust<'_>) -> io::Result<()> {
    apply_with_failure(root, plan, None, trust)
}

/// Resume an interrupted plan only when each target is still attributable to the journal.
///
/// A target already published before a crash but lacking committed ownership cannot be proven
/// to have been created by this writer, so recovery refuses it even when its digest matches.
///
/// # Errors
/// Returns an error for absent or malformed journals, ambiguous creates, and changed files.
pub fn recover(root: &Path, id: &str, trust: &CheckedTrust<'_>) -> io::Result<()> {
    let plan = super::recovery::read_journal(root, id, trust)?;
    apply_with_failure(root, &plan, None, trust)
}

/// Apply a plan with a test-only interruption point after each durable stage.
pub(crate) fn apply_with_failure(
    root: &Path,
    plan: &FilePlan,
    fail_after: Option<usize>,
    trust: &CheckedTrust<'_>,
) -> io::Result<()> {
    for file in &plan.entries {
        effects::check(root, &file.path, trust)?;
    }
    let journal = format!(".maestro-files/{}", journal_name(&plan.id));
    let journal_bytes = record_bytes(plan)?;
    let ownership_path = format!(".maestro-files/{}", ownership_name(&plan.id));
    effects::check(root, &journal, trust)?;
    effects::check(root, &ownership_path, trust)?;
    match read_optional(root, &ownership_path, trust)? {
        Some(existing) if ownership_matches(&existing, plan)? => {
            verify_owned_files(root, plan, trust)?;
            if read_optional(root, &journal, trust)?.is_some() {
                effects::remove(root, &journal, &journal_bytes, None, trust)?;
            }
            return Ok(());
        }
        Some(_) => {
            return Err(io::Error::new(
                ErrorKind::AlreadyExists,
                "conflicting ownership record",
            ));
        }
        None => {}
    }
    match read_optional(root, &journal, trust)? {
        Some(existing) if existing == journal_bytes => {}
        Some(_) => {
            return Err(io::Error::new(
                ErrorKind::AlreadyExists,
                "conflicting file journal",
            ));
        }
        None => write_new(
            root,
            (&journal, &journal_bytes),
            fail_after == Some(100),
            trust,
        )?,
    }
    fail_if_requested(fail_after, 0)?;

    let mut owned_files = Vec::with_capacity(plan.entries.len());
    for (index, file) in plan.entries.iter().enumerate() {
        match effects::read(root, &file.path, trust) {
            Ok(_) => {
                return Err(io::Error::new(
                    ErrorKind::AlreadyExists,
                    "planned target already exists; ownership cannot be proven",
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                fail_if_requested(fail_after, index * 2 + 1)?;
                let metadata = effects::write(root, &file.path, &file.bytes, trust)?;
                #[cfg(unix)]
                let identity = Some(file_identity(&metadata));
                #[cfg(windows)]
                let identity = {
                    let _ = metadata;
                    None
                };
                owned_files.push(OwnedFile {
                    path: &file.path,
                    digest: &file.digest,
                    identity,
                });
            }
            Err(error) => return Err(error),
        }
        fail_if_requested(fail_after, index * 2 + 2)?;
    }

    fail_if_requested(fail_after, plan.entries.len() * 2 + 1)?;
    let ownership_bytes = record_bytes(&Ownership {
        id: &plan.id,
        files: owned_files,
    })?;
    write_new(
        root,
        (&ownership_path, &ownership_bytes),
        fail_after == Some(101),
        trust,
    )?;
    fail_if_requested(fail_after, plan.entries.len() * 2 + 2)?;
    effects::remove(root, &journal, &journal_bytes, None, trust)?;
    Ok(())
}

/// Confirm every committed path still has the digest recorded by the plan.
fn verify_owned_files(root: &Path, plan: &FilePlan, trust: &CheckedTrust<'_>) -> io::Result<()> {
    for file in &plan.entries {
        match effects::read(root, &file.path, trust) {
            Ok(bytes) if digest(&bytes) == file.digest => {}
            Ok(_) => {
                return Err(io::Error::other(format!(
                    "owned file changed: {}",
                    file.path
                )));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Read stable identity from metadata of the newly-created file on Unix.
#[cfg(unix)]
fn file_identity(metadata: &Metadata) -> FileIdentity {
    use std::os::unix::fs::MetadataExt;

    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

/// Write a complete state record privately, then publish it atomically without replacement.
fn write_new(
    root: &Path,
    (name, bytes): (&str, &[u8]),
    tear: bool,
    trust: &CheckedTrust<'_>,
) -> io::Result<()> {
    let temporary = loop {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let (parent, leaf) = name
            .rsplit_once('/')
            .ok_or_else(|| io::Error::other("state path has no parent"))?;
        let temporary = format!("{parent}/.{leaf}.tmp-{}-{sequence}", process::id());
        let written = if tear {
            bytes.get(..bytes.len() / 2).unwrap_or_default()
        } else {
            bytes
        };
        match effects::write(root, &temporary, written, trust) {
            Ok(_) => break temporary,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    };
    if tear {
        return Err(io::Error::other("injected torn state-record write"));
    }
    let source = trust.authorize(root, Path::new(&temporary), Access::Read)?;
    trust
        .authorize(root, Path::new(name), Access::Write)?
        .publish_from(&source)?;
    effects::remove(root, &temporary, bytes, None, trust)
}

/// Stop at a named durable boundary in crash-contract tests.
fn fail_if_requested(fail_after: Option<usize>, point: usize) -> io::Result<()> {
    if fail_after == Some(point) {
        return Err(io::Error::other("injected file-operation interruption"));
    }
    Ok(())
}
