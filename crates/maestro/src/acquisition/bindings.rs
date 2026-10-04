//! Machine-local file bindings, never grant material or portable source configuration.
#[cfg(any(target_os = "linux", test))]
use super::command::resolve;
use crate::failure::Failure;
use maestro_acquisition::policy::resolve::parse_resource;
#[cfg(any(target_os = "linux", test))]
use maestro_acquisition::{
    Admission, AdmissionStatus, CheckedPolicy, DirectFiles, LocalResource, Principal, Ref,
};
#[cfg(any(target_os = "linux", test))]
use maestro_kernel::artifact::Digest;
#[cfg(any(target_os = "linux", test))]
use maestro_knowledge::collection::Declaration;
use maestro_knowledge::strict_json::MAX_BYTES;
use serde::de::DeserializeOwned;
#[cfg(any(target_os = "linux", test))]
use serde::{Deserialize, Serialize};
#[cfg(any(target_os = "linux", test))]
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use std::{fs::File, io::Read as _, path::Path};

/// Strict local resource binding wire record; no source-selected file lookup.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(any(target_os = "linux", test))]
struct Binding {
    /// Logical resource identity, unique within one bindings file.
    id: String,
    /// Caller-chosen path, relative only to the bindings file's directory.
    path: PathBuf,
    /// Separate existing immutable admission evidence, not entitlement.
    admission: AdmissionWire,
}
/// Existing Admission fields, with a closed wire spelling for its status.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(any(target_os = "linux", test))]
struct AdmissionWire {
    /// Exact reviewed original byte digest.
    digest: Digest,
    /// Qualified platform.
    platform: String,
    /// Qualified adapter capabilities.
    capabilities: Vec<String>,
    /// Only external reviewed evidence admits.
    status: ReviewStatus,
    /// Exact admitted immutable closure members.
    references: Vec<Ref>,
}
/// No strings or unknown status names can become a review.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg(any(target_os = "linux", test))]
enum ReviewStatus {
    /// Exact externally reviewed resource.
    Reviewed,
    /// Unreviewed proposal.
    Proposed,
    /// Failed or incomplete qualification.
    Held,
    /// Revoked external evidence.
    Revoked,
}
/// The single versioned local bindings decoder.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(any(target_os = "linux", test))]
struct Bindings {
    /// Supported version; other versions refuse before effects.
    schema: String,
    /// No maps that can silently overwrite duplicate IDs.
    resources: Vec<Binding>,
}
/// Bound bytes before the existing strict JSON decoder; no unbounded file reads.
pub(crate) fn read<T: DeserializeOwned>(path: &Path) -> Result<T, Failure> {
    parse_resource(&read_bytes(path)?).map_err(|_| Failure::refused("acquisition input invalid"))
}
/// Retain the exact portable manifest bytes, not a reserialization digest.
fn read_bytes(path: &Path) -> Result<Vec<u8>, Failure> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| Failure::refused("acquisition input unavailable"))?
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Failure::refused("acquisition input unavailable"))?;
    if bytes.len() > MAX_BYTES {
        return Err(Failure::refused("acquisition input invalid"));
    }
    Ok(bytes)
}
/// Use `DirectFiles`' existing path reads and the shared policy validator.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn load(
    manifest: &Path,
    bindings: &Path,
    principal: &Principal<'_>,
) -> Result<(CheckedPolicy, Digest), Failure> {
    let bytes = read_bytes(manifest)?;
    let collection: Declaration =
        parse_resource(&bytes).map_err(|_| Failure::refused("acquisition input invalid"))?;
    let (files, ids) = files(
        read(bindings)?,
        bindings
            .parent()
            .ok_or_else(|| Failure::refused("acquisition bindings invalid"))?,
    )?;
    let checked = resolve(&files, &collection, principal)?;
    let required: BTreeSet<_> = checked
        .references()
        .iter()
        .map(|reference| reference.id.clone())
        .collect();
    if ids != required {
        return Err(Failure::refused(
            "acquisition bindings contain unknown resources",
        ));
    }
    Ok((checked, Digest::of(&bytes)))
}
/// Refuse duplicate, empty and unsupported bindings before any starts.
#[cfg(any(target_os = "linux", test))]
fn files(wire: Bindings, base: &Path) -> Result<(DirectFiles, BTreeSet<String>), Failure> {
    if wire.schema != "maestro-acquisition-bindings/1"
        || wire.resources.is_empty()
        || wire.resources.len() > 1000
    {
        return Err(Failure::refused("acquisition bindings invalid"));
    }
    let mut resources = BTreeMap::new();
    for binding in wire.resources {
        let evidence = binding.admission;
        let status = match evidence.status {
            ReviewStatus::Reviewed => AdmissionStatus::Reviewed,
            ReviewStatus::Proposed => AdmissionStatus::Proposed,
            ReviewStatus::Held => AdmissionStatus::Held,
            ReviewStatus::Revoked => AdmissionStatus::Revoked,
        };
        let admission = Admission {
            digest: evidence.digest,
            platform: evidence.platform,
            capabilities: evidence.capabilities,
            status,
            references: evidence.references,
        };
        let path = if binding.path.is_absolute() {
            binding.path
        } else {
            base.join(binding.path)
        };
        if resources
            .insert(binding.id, LocalResource { path, admission })
            .is_some()
        {
            return Err(Failure::refused("acquisition bindings duplicate resource"));
        }
    }
    let ids = resources.keys().cloned().collect();
    Ok((DirectFiles::new(resources), ids))
}
#[cfg(test)]
mod tests {
    use super::{Bindings, files, parse_resource};
    use serde_json::Value;
    use std::{fs, path::Path};
    #[test]
    fn n14_bindings_golden_and_strict_decoder() {
        let bytes = include_bytes!("bindings-golden.json");
        let parsed: Bindings = parse_resource(bytes).unwrap();
        let expected: Value = serde_json::from_slice(bytes).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), expected);
        for bytes in [
            concat!(
                "{\"schema\":\"maestro-acquisition-bindings/1\",",
                "\"schema\":\"maestro-acquisition-bindings/1\",\"resources\":[]}"
            )
            .as_bytes(),
            b"{\"schema\":\"maestro-acquisition-bindings/1\",\"resources\":[],\"unknown\":1}",
        ] {
            assert!(parse_resource::<Bindings>(bytes).is_err());
        }
        let mut duplicated: Value = expected.clone();
        duplicated["resources"]
            .as_array_mut()
            .unwrap()
            .push(expected["resources"][0].clone());
        assert!(files(serde_json::from_value(duplicated).unwrap(), Path::new(".")).is_err());
        for length in [0, 1000, 1001] {
            let mut wire = expected.clone();
            wire["resources"] = serde_json::json!(
                (0..length)
                    .map(|index| {
                        let mut binding = expected["resources"][0].clone();
                        binding["id"] = serde_json::json!(format!("resource-{index}"));
                        binding
                    })
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                files(serde_json::from_value(wire).unwrap(), Path::new(".")).is_ok(),
                length == 1000
            );
        }
        let mut wrong = expected;
        wrong["schema"] = "unsupported".into();
        assert!(files(serde_json::from_value(wrong).unwrap(), Path::new(".")).is_err());
    }
    #[test]
    fn n14_direct_input_reads_are_bounded() {
        let root = maestro_test_scratch::scratch_directory().unwrap();
        let path = root.join("oversized.json");
        fs::write(&path, vec![b' '; super::MAX_BYTES]).unwrap();
        assert_eq!(super::read_bytes(&path).unwrap().len(), super::MAX_BYTES);
        fs::write(&path, vec![b' '; super::MAX_BYTES + 1]).unwrap();
        assert!(super::read_bytes(&path).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
