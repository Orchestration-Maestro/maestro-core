//! The pinned native tokenizer: verified artifacts and one counter process per input.
use super::artifacts::{verify_libraries, verify_record};
use super::binding::NativeBinding;
use super::contract::{CONTRACT_ID, array_at, invalid_contract, parse_contract, text_at, value_at};
use super::counter::TokenCounter;
use super::loader::Loader;
use super::process::run_native;
use crate::error::Error;
use serde_json::Value;
use std::{ffi::OsStr, path::PathBuf, process::Command, time::Duration};

/// Pinned local tokenizer. This type never runs a model forward pass.
///
/// The runtime installation is trusted, not protected from a hostile concurrent
/// writer. Reverify artifacts after a batch before accepting its token counts.
#[derive(Debug)]
pub struct NativeTokenizer {
    /// The committed qualification profile.
    pub(super) contract: Value,
    /// Where this machine keeps the profile's artifacts.
    pub(super) binding: NativeBinding,
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

    /// Recheck the pinned artifacts before accepting a batch.
    ///
    /// # Errors
    /// Refuses missing, nonregular or changed artifacts and redirected libraries.
    pub fn verify_artifacts(&self) -> Result<(), Error> {
        verify_record(
            &self.binding.model,
            value_at(&self.contract, "/artifacts/model")?,
        )?;
        verify_record(
            &self.binding.counter,
            value_at(&self.contract, "/artifacts/counter")?,
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
        self.verify_loading()
    }

    /// The libraries the counter loads are the verified ones: the directory holds exactly the
    /// profile's, and this platform's loader finds them before any other.
    fn verify_loading(&self) -> Result<(), Error> {
        verify_libraries(&self.library_paths()?)?;
        Loader::HOST.check_counter_location(&self.binding.counter, &self.binding.library_directory)
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

impl TokenCounter for NativeTokenizer {
    /// Identity of the qualified model, native implementation and preparation policy.
    fn contract_id(&self) -> &str {
        CONTRACT_ID
    }

    /// The artifact check, [`NativeTokenizer::verify_artifacts`].
    ///
    /// # Errors
    /// Refuses missing, nonregular or changed artifacts and redirected libraries.
    fn verify(&self) -> Result<(), Error> {
        self.verify_artifacts()
    }

    /// Tokenize complete UTF-8 input without padding or truncation.
    ///
    /// Call [`Self::verify_artifacts`] at batch boundaries. Executable bytes and
    /// library resolution are additionally checked before each process launch.
    ///
    /// # Errors
    /// Refuses changed executable/library resolution, process errors, timeouts,
    /// excessive process output and invalid ordered vocabulary IDs.
    fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error> {
        verify_record(
            &self.binding.counter,
            value_at(&self.contract, "/artifacts/counter")?,
        )?;
        self.verify_loading()?;
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
}

/// The counter command: the profile's arguments with the bound model in place,
/// the profile's environment only, and the bound library directory searched first.
pub(super) fn configured_command(
    contract: &Value,
    binding: &NativeBinding,
) -> Result<Command, Error> {
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
    // The counter names the directory it was built in (RUNPATH, @rpath); this platform's
    // loader variable is searched first, so the verified libraries are the loaded ones.
    let path = contract
        .pointer("/invocation/environment_replace/PATH")
        .and_then(Value::as_str);
    let (name, value) =
        Loader::HOST.search_variable(&binding.library_directory, path.map(OsStr::new))?;
    command.env(name, value);
    Ok(command)
}

/// The counter's output: a JSON array of vocabulary IDs that starts with BOS 0 and ends with EOS 2.
pub(super) fn parse_ids(bytes: &[u8]) -> Result<Vec<u32>, Error> {
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
