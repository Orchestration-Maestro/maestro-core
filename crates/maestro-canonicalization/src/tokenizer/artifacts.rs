//! Artifact checks: pinned files by size and SHA-256, and the exact library inventory.
use super::contract::{invalid_contract, text_at, value_at};
use crate::error::Error;
use crate::hashing::lower_hex;
use rustix::fs::{Mode, OFlags, open};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
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
/// following links.
pub(super) fn verify_artifact(path: &Path, bytes: u64, hash: &str) -> Result<(), Error> {
    let fd = open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|_| Error("tokenizer artifact unavailable".into()))?;
    let mut file = File::from(fd);
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

/// The library directory holds exactly the profile's libraries and their version aliases, each
/// resolving to its pinned file.
pub(super) fn verify_libraries(libraries: &[PathBuf]) -> Result<(), Error> {
    let mut expected = BTreeMap::new();
    for path in libraries {
        let directory = path.parent().ok_or_else(invalid_contract)?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(invalid_contract)?;
        let (base, suffix) = name.split_once(".so").ok_or_else(invalid_contract)?;
        expected.insert(path.clone(), path.clone());
        if !suffix.is_empty() {
            let major = suffix
                .strip_prefix('.')
                .and_then(|suffix| suffix.split('.').next())
                .ok_or_else(invalid_contract)?;
            expected.insert(directory.join(format!("{base}.so")), path.clone());
            expected.insert(directory.join(format!("{base}.so.{major}")), path.clone());
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
        if entry.file_name().to_string_lossy().contains(".so") {
            actual.insert(entry.path());
        }
    }
    if actual != expected.keys().cloned().collect() {
        return Err(Error("tokenizer library inventory mismatch".into()));
    }
    for (alias, target) in &expected {
        if fs::canonicalize(alias)
            .map_err(|_| Error("tokenizer library resolution failed".into()))?
            != *target
        {
            return Err(Error("tokenizer library target mismatch".into()));
        }
    }
    Ok(())
}
