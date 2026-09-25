//! Artifact checks: pinned files by size and SHA-256, and the exact library inventory.
use super::contract::{invalid_contract, text_at, value_at};
use crate::error::Error;
use crate::filesystem::open_nofollow;
use crate::hashing::lower_hex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

/// Check a bound artifact against its profile record's size and SHA-256.
pub(super) fn verify_record(path: &Path, record: &Value) -> Result<(), Error> {
    verify_artifact(
        path,
        value_at(record, "/bytes")?
            .as_u64()
            .ok_or_else(invalid_contract)?,
        text_at(record, "/sha256")?,
    )
}

/// Refuse a path that is not a regular file of exactly this size and SHA-256, read without
/// following a link in its last component and without blocking on a FIFO.
pub(super) fn verify_artifact(path: &Path, bytes: u64, hash: &str) -> Result<(), Error> {
    let mut file =
        open_nofollow(path).map_err(|_| Error("tokenizer artifact unavailable".into()))?;
    let metadata = file
        .metadata()
        .map_err(|_| Error("tokenizer artifact metadata unavailable".into()))?;
    if !metadata.is_file() || metadata.len() != bytes {
        return Err(Error(
            "tokenizer artifact size or file type mismatch".into(),
        ));
    }
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let length = file
            .read(&mut buffer)
            .map_err(|_| Error("tokenizer artifact read failed".into()))?;
        if length == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(length).map_err(|_| invalid_contract())?)
            .ok_or_else(invalid_contract)?;
        hasher.update(buffer.get(..length).ok_or_else(invalid_contract)?);
    }
    if total != bytes || lower_hex(&hasher.finalize()) != hash {
        return Err(Error("tokenizer artifact fingerprint mismatch".into()));
    }
    Ok(())
}

/// The names a pinned library is also reached by, under the shared-library naming its own file
/// name follows, so that every host applies the same rule to the same profile:
/// - Linux: `libx.so.1.2` is linked as `libx.so` and `libx.so.1`;
/// - macOS: `libx.1.2.dylib` is linked as `libx.dylib` and `libx.1.dylib`;
/// - Windows: `x.dll` has none, and neither has an unversioned `libx.so` or `libx.dylib`.
///
/// The macOS and Windows extensions match in any letter case, as their file systems do.
pub(super) fn version_aliases(name: &str) -> Result<Vec<String>, Error> {
    let (stem, extension) = name.rsplit_once('.').ok_or_else(invalid_contract)?;
    if extension.eq_ignore_ascii_case("dll") {
        return Ok(Vec::new());
    }
    if extension.eq_ignore_ascii_case("dylib") {
        return Ok(stem
            .split_once('.')
            .map_or_else(Vec::new, |(base, version)| {
                let major = version.split_once('.').map_or(version, |(major, _)| major);
                vec![
                    format!("{base}.{extension}"),
                    format!("{base}.{major}.{extension}"),
                ]
            }));
    }
    let (base, suffix) = name.split_once(".so").ok_or_else(invalid_contract)?;
    if suffix.is_empty() {
        return Ok(Vec::new());
    }
    let major = suffix
        .strip_prefix('.')
        .and_then(|suffix| suffix.split('.').next())
        .ok_or_else(invalid_contract)?;
    Ok(vec![format!("{base}.so"), format!("{base}.so.{major}")])
}

/// Whether a directory entry is a shared library under any platform's naming: a name holding
/// `.so`, or a `.dylib` or `.dll` extension in any letter case.
fn is_library(name: &str) -> bool {
    let extension = name.rsplit_once('.').map_or("", |(_, extension)| extension);
    name.contains(".so")
        || extension.eq_ignore_ascii_case("dylib")
        || extension.eq_ignore_ascii_case("dll")
}

/// The library directory holds exactly the profile's libraries and their version aliases, each
/// resolving to its pinned file. Both sides are compared resolved, so a directory reached through
/// a link (macOS `/var`) or named without the `\\?\` prefix Windows resolution adds passes alike.
pub(super) fn verify_libraries(libraries: &[PathBuf]) -> Result<(), Error> {
    let mut expected = BTreeMap::new();
    for path in libraries {
        let directory = path.parent().ok_or_else(invalid_contract)?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(invalid_contract)?;
        expected.insert(path.clone(), name);
        for alias in version_aliases(name)? {
            expected.insert(directory.join(alias), name);
        }
    }
    let directory = expected
        .keys()
        .next()
        .and_then(|path| path.parent())
        .ok_or_else(invalid_contract)?;
    if expected.keys().any(|path| path.parent() != Some(directory)) {
        return Err(invalid_contract());
    }
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(directory)
        .map_err(|_| Error("tokenizer library directory unavailable".into()))?
    {
        let entry = entry.map_err(|_| Error("tokenizer library inventory unavailable".into()))?;
        if is_library(&entry.file_name().to_string_lossy()) {
            actual.insert(entry.path());
        }
    }
    if actual != expected.keys().cloned().collect() {
        return Err(Error("tokenizer library inventory mismatch".into()));
    }
    // The pinned file itself must resolve to its own name here: it is no link to elsewhere.
    let resolved = fs::canonicalize(directory)
        .map_err(|_| Error("tokenizer library resolution failed".into()))?;
    for (alias, target) in &expected {
        if fs::canonicalize(alias)
            .map_err(|_| Error("tokenizer library resolution failed".into()))?
            != resolved.join(target)
        {
            return Err(Error("tokenizer library target mismatch".into()));
        }
    }
    Ok(())
}
