//! Local vocabulary-only tokenization through the qualified executable.
use self::binding::NativeBinding;
use self::process::run_native;
use crate::{Error, digest};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

mod binding;
mod process;
#[cfg(test)]
mod tests;

/// The committed profile's identifier: the SHA-256 of its canonical JSON without this field.
const CONTRACT_ID: &str = "sha256:3546447555757daa4996a2e2e708bc67bce4389e8cee3f8386ed503eeaa6d01c";

/// Pinned local tokenizer. This type never runs a model forward pass.
///
/// The runtime installation is trusted, not protected from a hostile concurrent
/// writer. Reverify artifacts after a batch before accepting its token counts.
#[derive(Debug)]
pub struct NativeTokenizer {
    /// The committed qualification profile.
    contract: Value,
    /// Where this machine keeps the profile's artifacts.
    binding: NativeBinding,
}

impl NativeTokenizer {
    /// Verify the approved profile and its artifacts where
    /// `MAESTRO_NATIVE_BINDING` says they are.
    ///
    /// # Errors
    /// Refuses a missing or invalid binding, unavailable or changed artifacts
    /// and invalid profiles.
    pub fn open() -> Result<Self, Error> {
        let counter = Self {
            contract: parse_contract(include_str!("../../tokenizer-contract.json"))?,
            binding: NativeBinding::from_environment()?,
        };
        counter.verify_artifacts()?;
        Ok(counter)
    }

    /// Identity of the qualified model, native implementation and preparation policy.
    #[must_use]
    pub fn contract_id(&self) -> &'static str {
        CONTRACT_ID
    }

    /// Tokenize complete UTF-8 input without padding or truncation.
    ///
    /// Call [`Self::verify_artifacts`] at batch boundaries. Executable bytes and
    /// library resolution are additionally checked before each process launch.
    ///
    /// # Errors
    /// Refuses changed executable/library resolution, process errors, timeouts,
    /// excessive process output and invalid ordered vocabulary IDs.
    pub fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error> {
        verify_record(
            &self.binding.counter,
            &self.contract["artifacts"]["counter"],
        )?;
        verify_libraries(&self.library_paths()?)?;
        let mut command = configured_command(&self.contract, &self.binding)?;
        let timeout = self
            .contract
            .pointer("/invocation/timeout_seconds")
            .and_then(Value::as_u64)
            .ok_or_else(invalid_contract)?;
        // ponytail: reload vocabulary per call; use a separately qualified persistent
        // adapter if measured throughput requires it.
        parse_ids(&run_native(
            &mut command,
            input.as_bytes(),
            Duration::from_secs(timeout),
        )?)
    }

    /// Recheck the pinned artifacts before accepting a batch.
    ///
    /// # Errors
    /// Refuses missing, nonregular or changed artifacts and redirected libraries.
    pub fn verify_artifacts(&self) -> Result<(), Error> {
        verify_record(&self.binding.model, &self.contract["artifacts"]["model"])?;
        verify_record(
            &self.binding.counter,
            &self.contract["artifacts"]["counter"],
        )?;
        for source in array_at(&self.contract, "/artifacts/sources")? {
            verify_record(
                &self.binding.source_root.join(text_at(source, "/file")?),
                source,
            )?;
        }
        for library in array_at(&self.contract, "/artifacts/libraries")? {
            let path = self
                .binding
                .library_directory
                .join(text_at(library, "/file")?);
            verify_record(&path, library)?;
        }
        verify_libraries(&self.library_paths()?)
    }

    /// The profile's libraries inside the bound directory.
    fn library_paths(&self) -> Result<Vec<PathBuf>, Error> {
        array_at(&self.contract, "/artifacts/libraries")?
            .iter()
            .map(|library| {
                Ok(self
                    .binding
                    .library_directory
                    .join(text_at(library, "/file")?))
            })
            .collect()
    }
}

/// The refusal for a profile that lacks a field or has the wrong type.
fn invalid_contract() -> Error {
    Error("invalid or unqualified tokenizer contract".into())
}

/// The string at a JSON pointer in the profile.
fn text_at<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, Error> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(invalid_contract)
}

/// The array at a JSON pointer in the profile.
fn array_at<'a>(value: &'a Value, pointer: &str) -> Result<&'a [Value], Error> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(invalid_contract)
}

/// The JSON value with object keys sorted at every level, for one canonical serialization.
fn sorted(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let ordered: BTreeMap<_, _> = map
                .into_iter()
                .map(|(key, value)| (key, sorted(value)))
                .collect();
            Value::Object(ordered.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

/// Parse the profile: its declared identifier must equal the compiled one and the digest of its
/// sorted JSON without that field.
fn parse_contract(text: &str) -> Result<Value, Error> {
    let mut value: Value = serde_json::from_str(text).map_err(|_| invalid_contract())?;
    let declared = value
        .as_object_mut()
        .and_then(|map| map.remove("contract_id"))
        .ok_or_else(invalid_contract)?;
    if declared.as_str() != Some(CONTRACT_ID) {
        return Err(invalid_contract());
    }
    let mut value = sorted(value);
    let bytes = serde_json::to_vec(&value).map_err(|_| invalid_contract())?;
    if format!("sha256:{}", digest(&bytes)) != CONTRACT_ID {
        return Err(invalid_contract());
    }
    value["contract_id"] = declared;
    Ok(value)
}

/// The counter command: the profile's arguments with the bound model in place,
/// the profile's environment only, and the bound library directory to load from.
fn configured_command(contract: &Value, binding: &NativeBinding) -> Result<Command, Error> {
    let mut command = Command::new(&binding.counter);
    for argument in array_at(contract, "/invocation/args")? {
        let argument = argument.as_str().ok_or_else(invalid_contract)?;
        if argument == "{model}" {
            command.arg(&binding.model);
        } else {
            command.arg(argument);
        }
    }
    command.env_clear();
    for (name, value) in contract
        .pointer("/invocation/environment_replace")
        .and_then(Value::as_object)
        .ok_or_else(invalid_contract)?
    {
        command.env(name, value.as_str().ok_or_else(invalid_contract)?);
    }
    // The counter's RUNPATH names the directory it was built in; the loader
    // searches LD_LIBRARY_PATH first, so the verified libraries are the loaded ones.
    command.env("LD_LIBRARY_PATH", &binding.library_directory);
    Ok(command)
}

/// The counter's output: a JSON array of vocabulary IDs that starts with BOS 0 and ends with EOS 2.
fn parse_ids(bytes: &[u8]) -> Result<Vec<u32>, Error> {
    let ids: Vec<u32> =
        serde_json::from_slice(bytes).map_err(|_| Error("invalid tokenizer output".into()))?;
    if ids.first() != Some(&0)
        || ids.last() != Some(&2)
        || ids.len() < 2
        || ids.iter().any(|&id| id >= 250_002)
    {
        return Err(Error("invalid tokenizer IDs or special tokens".into()));
    }
    Ok(ids)
}

/// Check a bound artifact against its profile record's size and SHA-256.
fn verify_record(path: &Path, record: &Value) -> Result<(), Error> {
    verify_artifact(
        path,
        record["bytes"].as_u64().ok_or_else(invalid_contract)?,
        text_at(record, "/sha256")?,
    )
}

/// Refuse a path that is not a regular file of exactly this size and SHA-256, read without
/// following links.
fn verify_artifact(path: &Path, bytes: u64, hash: &str) -> Result<(), Error> {
    use rustix::fs::{Mode, OFlags};
    let fd = rustix::fs::open(
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
        hasher.update(&buffer[..length]);
    }
    if total != bytes || format!("{:x}", hasher.finalize()) != hash {
        return Err(Error("tokenizer artifact fingerprint mismatch".into()));
    }
    Ok(())
}

/// The library directory holds exactly the profile's libraries and their version aliases, each
/// resolving to its pinned file.
fn verify_libraries(libraries: &[PathBuf]) -> Result<(), Error> {
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
