//! Local overlay I/O and protected receipt identity helpers.
use super::manifest::{Activation, Proposal, WriteError};
use crate::{Principal, Ref, Refusal};
use maestro_kernel::{
    acquisition::{Handle, Progress, Receipts},
    artifact::Digest,
    filesystem,
    scope::Scope,
};
use maestro_knowledge::strict_json;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read as _},
    path::Path,
};

/// Durable before/after pointer digests; payloads live in scoped receipt storage.
#[derive(Debug, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Recovery {
    /// Complete old pointer identity.
    pub old: Digest,
    /// Complete new pointer identity.
    pub new: Digest,
    /// Absent for proposal-only commits; activation report handle otherwise.
    #[serde(deserialize_with = "Option::deserialize")]
    pub event: Option<Progress>,
}

/// Locks the collection overlay on a protected regular caller-owned lock file.
pub(super) fn lock(root: &Path) -> Result<File, WriteError> {
    filesystem::protected_root(root)?;
    let path = root.join("writer.lock");
    match filesystem::new_file().open(&path) {
        Ok(file) => {
            drop(file);
            filesystem::sync_directory(root, |_, error| error)?;
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    regular(&path)?;
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    file.lock()?;
    Ok(file)
}

/// Never follow a local pointer/recovery/lock symlink or block on a pipe.
fn regular(path: &Path) -> Result<(), WriteError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() {
        return Err(Refusal::Invalid.into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(Refusal::Invalid.into());
        }
    }
    Ok(())
}

/// Bounded original pointer bytes; absent is distinct from corrupt.
pub(super) fn read(root: &Path, name: &str) -> Result<Option<Vec<u8>>, WriteError> {
    let path = root.join(name);
    if let Err(error) = fs::symlink_metadata(&path) {
        return if error.kind() == ErrorKind::NotFound {
            Ok(None)
        } else {
            Err(error.into())
        };
    }
    regular(&path)?;
    let mut bytes = Vec::new();
    File::open(path)?
        .take((strict_json::MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

/// Strict, bounded wire decoding is shared with N03, including duplicate keys.
pub(super) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, WriteError> {
    strict_json::parse(bytes).map_err(|_| Refusal::Invalid.into())
}

/// Deterministic canonical JSON with lexically ordered object fields.
pub(super) fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, WriteError> {
    let value = serde_json::to_value(value).map_err(|_| WriteError::Refused(Refusal::Invalid))?;
    let bytes = serde_json::to_vec(&value).map_err(|_| WriteError::Refused(Refusal::Invalid))?;
    Ok(bytes)
}

/// Read every input handle using fresh kernel grants, including transitive scopes.
pub(super) fn accessible(
    receipts: &dyn Receipts,
    principal: &Principal<'_>,
    handles: &[Handle],
) -> Result<(), WriteError> {
    for handle in handles {
        if receipts
            .read(principal.id, *handle)
            .map_err(|_| WriteError::Storage)?
            .is_none()
        {
            return Err(Refusal::Access.into());
        }
    }
    Ok(())
}

/// Store a payload with every inherited scope enforced through N06's link closure.
pub(super) fn retain(
    receipts: &dyn Receipts,
    tags: &[String],
    bytes: &[u8],
    references: &[Handle],
) -> Result<Ref, WriteError> {
    let mut links = references.to_vec();
    let mut scopes = tags.iter();
    let primary: Scope = scopes
        .next()
        .ok_or(Refusal::Access)?
        .parse()
        .map_err(|_| Refusal::Invalid)?;
    for tag in scopes {
        let scope: Scope = tag.parse().map_err(|_| Refusal::Invalid)?;
        links.push(
            receipts
                .retain(&scope, b"{}", &[])
                .map_err(|_| WriteError::Storage)?,
        );
    }
    let handle = receipts
        .retain(&primary, bytes, &links)
        .map_err(|_| WriteError::Storage)?;
    Ok(Ref {
        id: handle.to_string(),
        digest: Digest::of(bytes),
    })
}

/// Load a digest-bound immutable overlay through current access checks only.
pub(super) fn artifact<T: DeserializeOwned>(
    receipts: &dyn Receipts,
    principal: &Principal<'_>,
    reference: &Ref,
) -> Result<T, WriteError> {
    let handle = reference.id.parse().map_err(|_| Refusal::Invalid)?;
    let resource = receipts
        .read(principal.id, handle)
        .map_err(|_| WriteError::Storage)?
        .ok_or(Refusal::Access)?;
    if Digest::of(resource.bytes()) != reference.digest {
        return Err(Refusal::Digest.into());
    }
    decode(resource.bytes())
}

/// Gate and proposal report/evidence handles must remain currently readable.
pub(super) fn inputs(
    receipts: &dyn Receipts,
    principal: &Principal<'_>,
    proposal: &Proposal,
    gate: Handle,
) -> Result<(), WriteError> {
    let mut handles = proposal.evidence.clone();
    handles.extend([proposal.report, gate]);
    accessible(receipts, principal, &handles)
}

/// Load the activation's exact proposal, retaining all original report bindings.
pub(super) fn proposal(
    receipts: &dyn Receipts,
    principal: &Principal<'_>,
    activation: &Activation,
) -> Result<Proposal, WriteError> {
    artifact(receipts, principal, &activation.proposal)
}

/// Clear recovery only after delivery or proven uncommitted staging.
pub(super) fn clear(root: &Path) -> Result<(), WriteError> {
    fs::remove_file(root.join("recovery.json"))?;
    filesystem::sync_directory(root, |_, error| error)?;
    Ok(())
}
