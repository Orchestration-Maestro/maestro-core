//! Caller-bound direct files, never source-provided paths or writable bundles.
use crate::{
    policy::resolve::validate,
    ports::{Admission, CheckedPolicy, ImmutableResource, PolicySource, Principal, ResourceSource},
    refusal::Refusal,
};
use maestro_knowledge::collection::Declaration;
use maestro_knowledge::collection::PolicyReference as Ref;
use maestro_knowledge::strict_json::MAX_BYTES;
use std::{collections::BTreeMap, fs::File, io::Read as _, path::PathBuf};

/// A trusted local binding and its separate reviewed evidence.
#[derive(Debug, Clone)]
pub struct LocalResource {
    /// Caller-selected file, outside policy configuration.
    pub path: PathBuf,
    /// Immutable reviewed digest and qualification context.
    pub admission: Admission,
}

/// Direct-file adapter; digest verification is shared with catalog adapters.
#[derive(Debug)]
pub struct DirectFiles(BTreeMap<String, LocalResource>);
impl DirectFiles {
    /// Bind logical IDs to externally authorized files and summaries.
    #[must_use]
    pub fn new(resources: BTreeMap<String, LocalResource>) -> Self {
        Self(resources)
    }
}
impl ResourceSource for DirectFiles {
    fn read(
        &self,
        reference: &Ref,
        _principal: &Principal<'_>,
    ) -> Result<ImmutableResource, Refusal> {
        let binding = self.0.get(&reference.id).ok_or(Refusal::Missing)?;
        let mut bytes = Vec::new();
        File::open(&binding.path)
            .map_err(|_| Refusal::Missing)?
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| Refusal::Missing)?;
        if bytes.len() > MAX_BYTES {
            return Err(Refusal::Invalid);
        }
        Ok(ImmutableResource {
            reference: reference.clone(),
            bytes,
            admission: binding.admission.clone(),
        })
    }
}
impl PolicySource for DirectFiles {
    fn resolve(
        &self,
        collection: &Declaration,
        principal: &Principal<'_>,
    ) -> Result<CheckedPolicy, Refusal> {
        validate(self, collection, principal)
    }
}
