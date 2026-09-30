//! Apply exclusive file plans, commit ownership last, and recover proven states.
use super::{
    plan::{FilePlan, digest, split_path},
    recovery::{journal_name, ownership_name, read_optional, record_bytes, state_directory},
};
use maestro_filesystem::Directory;
use serde::Serialize;
use std::{
    fs::File,
    io::{self, ErrorKind, Write},
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
}

/// Apply a previewed plan, publishing ownership only after every exclusive create is durable.
///
/// A target that exists without a committed ownership record is refused, even when its bytes
/// match the plan. Recovery cannot prove who created a target if a crash occurred after its create
/// but before ownership was committed; that target is retained for an explicit user decision.
///
/// # Errors
/// Returns an error for an existing or changed target, journal conflict, or filesystem failure.
pub fn apply(root: &Path, plan: &FilePlan) -> io::Result<()> {
    apply_with_failure(root, plan, None)
}

/// Resume an interrupted plan only when each target is still attributable to the journal.
///
/// A target already published before a crash but lacking committed ownership cannot be proven
/// to have been created by this writer, so recovery refuses it even when its digest matches.
///
/// # Errors
/// Returns an error for absent or malformed journals, ambiguous creates, and changed files.
pub fn recover(root: &Path, id: &str) -> io::Result<()> {
    let plan = super::recovery::read_journal(root, id)?;
    apply_with_failure(root, &plan, None)
}

/// Apply a plan with a test-only interruption point after each durable stage.
pub(super) fn apply_with_failure(
    root: &Path,
    plan: &FilePlan,
    fail_after: Option<usize>,
) -> io::Result<()> {
    let state = state_directory(root)?;
    let journal = journal_name(&plan.id);
    let journal_bytes = record_bytes(plan)?;
    let ownership = ownership_record(plan);
    let ownership_bytes = record_bytes(&ownership)?;
    let ownership_path = ownership_name(&plan.id);
    match read_optional(&state, &ownership_path)? {
        Some(existing) if existing == ownership_bytes => {
            verify_owned_files(root, plan)?;
            if read_optional(&state, &journal)?.is_some() {
                state.remove_verified(&journal, &journal_bytes)?;
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
    match read_optional(&state, &journal)? {
        Some(existing) if existing == journal_bytes => {}
        Some(_) => {
            return Err(io::Error::new(
                ErrorKind::AlreadyExists,
                "conflicting file journal",
            ));
        }
        None => write_new(&state, &journal, &journal_bytes, fail_after == Some(100))?,
    }
    fail_if_requested(fail_after, 0)?;

    for (index, file) in plan.entries.iter().enumerate() {
        let (parent, name) = split_path(&file.path)?;
        let directory = Directory::open(root, &parent, true)?;
        match directory.read_regular(name) {
            Ok(_) => {
                return Err(io::Error::new(
                    ErrorKind::AlreadyExists,
                    "planned target already exists; ownership cannot be proven",
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                fail_if_requested(fail_after, index * 2 + 1)?;
                let mut output = directory.create_new(name)?;
                output.write_all(&file.bytes)?;
                output.sync_all()?;
            }
            Err(error) => return Err(error),
        }
        fail_if_requested(fail_after, index * 2 + 2)?;
    }

    fail_if_requested(fail_after, plan.entries.len() * 2 + 1)?;
    write_new(
        &state,
        &ownership_path,
        &ownership_bytes,
        fail_after == Some(101),
    )?;
    fail_if_requested(fail_after, plan.entries.len() * 2 + 2)?;
    state.remove_file(&journal)?;
    Ok(())
}

/// Confirm every committed path still has the digest recorded by the plan.
fn verify_owned_files(root: &Path, plan: &FilePlan) -> io::Result<()> {
    for file in &plan.entries {
        let (parent, name) = split_path(&file.path)?;
        let directory = Directory::open(root, &parent, false)?;
        match directory.read_regular(name) {
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

/// Build the ownership payload published after every file write.
fn ownership_record(plan: &FilePlan) -> Ownership<'_> {
    Ownership {
        id: &plan.id,
        files: plan
            .entries
            .iter()
            .map(|entry| OwnedFile {
                path: &entry.path,
                digest: &entry.digest,
            })
            .collect(),
    }
}

/// Write a complete state record privately, then publish it atomically without replacement.
fn write_new(directory: &Directory, name: &str, bytes: &[u8], tear: bool) -> io::Result<()> {
    let (temporary, mut file): (String, File) = loop {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let temporary = format!(".{name}.tmp-{}-{sequence}", process::id());
        match directory.create_new(&temporary) {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    directory.sync()?;
    if tear {
        file.write_all(&bytes[..bytes.len() / 2])?;
        file.sync_all()?;
        return Err(io::Error::other("injected torn state-record write"));
    }
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    directory.link(&temporary, name)?;
    directory.remove_file(&temporary)
}

/// Stop at a named durable boundary in crash-contract tests.
fn fail_if_requested(fail_after: Option<usize>, point: usize) -> io::Result<()> {
    if fail_after == Some(point) {
        return Err(io::Error::other("injected file-operation interruption"));
    }
    Ok(())
}
