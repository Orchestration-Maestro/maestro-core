//! The only archive-library adapter; compilation never executes source content.

use super::manifest::{Budget, Bundle, Entry, Manifest, refuse};
use crate::files::digest;
use crate::{
    limits::Limits,
    policy::schema::bound_json,
    source::{Known, Refusal, Registry, SourceTree, checked_snapshot},
};
use serde::Serialize;
use std::{collections::BTreeMap, str};
use tar::{Builder, EntryType, Header};

/// Compile one checked snapshot into an inert deterministic archive. All limits
/// include `bundle.json`; the tar stream includes headers, padding and end blocks.
/// Nothing is published by this function, including on refusal.
///
/// # Errors
/// Source refusals, invalid provenance, or any archive limit violation.
pub fn compile(
    tree: &dyn SourceTree,
    registry: &Registry,
    limits: &Limits,
    known: Known<'_>,
    source_commit: &str,
) -> Result<Bundle, Refusal> {
    if ![40, 64].contains(&source_commit.len())
        || !source_commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(refuse(
            "source_commit",
            "use a full 40 or 64 character lowercase hex source commit",
        ));
    }
    let (catalog, snapshot) = checked_snapshot(tree, registry, limits, known)?;
    if snapshot.paths().count().saturating_add(1) > limits.archive_entries {
        return Err(refuse("bundle.json", "archive entry count exceeds limit"));
    }
    let (mut manifest, mut budget) = Manifest::build(&catalog, registry, limits, source_commit)?;
    let mut files = BTreeMap::new();
    let mut aggregate = 0;
    let mut stream = 1024;
    for path in snapshot.paths() {
        let bytes = snapshot
            .read(path, limits.source_file_bytes)
            .map_err(|error| refuse(path, error.to_string()))?;
        charge(
            path,
            bytes.len() as u64,
            limits,
            &mut aggregate,
            &mut stream,
        )?;
        let entry = Entry {
            digest: digest(&bytes),
            bytes: bytes.len() as u64,
        };
        budget.charge(path)?;
        budget.charge(&entry)?;
        manifest.entries.insert(path.clone(), entry);
        files.insert(path.clone(), bytes);
    }
    let bytes = canonical(&manifest, limits)?;
    charge(
        "bundle.json",
        bytes.len() as u64,
        limits,
        &mut aggregate,
        &mut stream,
    )?;
    files.insert("bundle.json".to_owned(), bytes);
    if stream > limits.archive_total_bytes {
        return Err(refuse("bundle.json", "archive stream bytes exceed limit"));
    }
    let bytes = archive(files, stream)?;
    Ok(Bundle {
        digest: digest(&bytes),
        bytes,
        manifest,
    })
}

/// Preflight payload and full ustar stream independently, using checked arithmetic.
pub(super) fn charge(
    path: &str,
    length: u64,
    limits: &Limits,
    aggregate: &mut u64,
    stream: &mut u64,
) -> Result<(), Refusal> {
    if length > limits.archive_entry_bytes {
        return Err(refuse(path, "archive entry bytes exceed limit"));
    }
    *aggregate = aggregate
        .checked_add(length)
        .ok_or_else(|| refuse(path, "aggregate bytes overflow"))?;
    if *aggregate > limits.archive_total_bytes {
        return Err(refuse(path, "archive aggregate bytes exceed limit"));
    }
    let padded = length
        .checked_add(511)
        .map(|n| n / 512 * 512)
        .and_then(|n| n.checked_add(512))
        .ok_or_else(|| refuse(path, "stream bytes overflow"))?;
    *stream = stream
        .checked_add(padded)
        .ok_or_else(|| refuse(path, "stream bytes overflow"))?;
    // Aggregate is checked first so its distinct refusal remains observable.
    Ok(())
}

/// Count bytes before allocation, then check depth before constructing a JSON
/// tree. `sort_all_objects` is required because the workspace enables `preserve_order`.
fn canonical(manifest: &impl Serialize, limits: &Limits) -> Result<Vec<u8>, Refusal> {
    Budget::new(limits.archive_entry_bytes).charge(manifest)?;
    let bytes =
        serde_json::to_vec(manifest).map_err(|error| refuse("bundle.json", error.to_string()))?;
    let text = str::from_utf8(&bytes).map_err(|error| refuse("bundle.json", error.to_string()))?;
    let json_limits = Limits {
        source_file_bytes: limits.archive_entry_bytes,
        source_depth: limits.manifest_depth,
        ..*limits
    };
    bound_json(text, &json_limits)
        .map_err(|error| refuse("bundle.json", format!("manifest nesting: {error}")))?;
    let mut value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|error| refuse("bundle.json", error.to_string()))?;
    value.sort_all_objects();
    serde_json::to_vec(&value).map_err(|error| refuse("bundle.json", error.to_string()))
}

/// Encode only regular files, in sorted order, using ustar (no extra headers).
fn archive(files: BTreeMap<String, Vec<u8>>, stream: u64) -> Result<Vec<u8>, Refusal> {
    let capacity =
        usize::try_from(stream).map_err(|error| refuse("bundle.json", error.to_string()))?;
    let mut builder = Builder::new(Vec::with_capacity(capacity));
    for (path, bytes) in files {
        let mut header = Header::new_ustar();
        header
            .set_path(&path)
            .map_err(|error| refuse(&path, format!("path cannot fit ustar: {error}")))?;
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_entry_type(EntryType::Regular);
        header.set_cksum();
        builder
            .append(&header, bytes.as_slice())
            .map_err(|error| refuse(&path, error.to_string()))?;
    }
    builder
        .into_inner()
        .map_err(|error| refuse("bundle.json", error.to_string()))
}
