//! Independently authored synthetic registry resources, never catalog trust.
#![expect(
    clippy::indexing_slicing,
    reason = "independently authored fixture fields must exist"
)]
use super::support::{Catalog, fixture, principal, put, rebind, scopes};
use maestro_acquisition::extraction::{
    detect::Observation,
    registry::{Profile, definition_bytes},
};
use maestro_acquisition::{AdmissionStatus, DirectFiles, LocalResource, Ref};
use maestro_kernel::artifact::Digest;
use maestro_test_scratch::scratch_directory;
use serde_json::{Value, json};
use std::{collections::BTreeMap, env, fs, path::PathBuf};

/// Synthetic closure with exact profile definition bytes and separate admission.
pub(super) fn registry_fixture() -> (Value, Catalog, Ref) {
    let (mut collection, mut catalog) = fixture();
    for id in ["gold", "threshold", "plugin", "output"] {
        put(&mut catalog, id, &json!({"synthetic":id}));
    }
    catalog.0.get_mut("gold").unwrap().admission.capabilities = vec!["gold".into()];
    catalog
        .0
        .get_mut("threshold")
        .unwrap()
        .admission
        .capabilities = vec!["thresholds".into()];
    catalog.0.get_mut("plugin").unwrap().admission.capabilities = vec!["synthetic-rust".into()];
    // The new processing pins also participate in every fixture rebind.
    for id in ["cleanup-default", "synthetic-chunk", "dedup-default"] {
        put(&mut catalog, id, &json!({"synthetic":id}));
        let resource = catalog.0.get_mut(id).unwrap();
        resource.bytes = id.as_bytes().to_vec();
        let digest = Digest::of(&resource.bytes);
        resource.reference.digest = digest.clone();
        resource.admission.digest = digest;
    }
    let mut profiles = Vec::new();
    for (id, detectors) in [
        ("safe", json!([])),
        (
            "markdown",
            json!([{"kind":"magic","offset":0,"hex_bytes":"2320"}]),
        ),
        (
            "novel",
            json!([
                {"kind":"magic","offset":0,"hex_bytes":"4e4557"},
                {"kind":"parser_capability","id":"synthetic-rust"}
            ]),
        ),
    ] {
        let mut value = json!({
            "id":id,"version":1,"definition_digest":"0".repeat(64),
            "detectors":detectors,"structures":[],"languages":["en"],
            "extractor":catalog.0["plugin"].reference,
            "processing":{
                "cleanup":{"id":"cleanup-default",
                    "digest":"fad6883aa083042e3e278ca312007bbd9f222bdb7ce11bc8ebdc7306205b1935"},
                "chunk":{"id":"synthetic-chunk",
                    "digest":"c5304fd6195251d9fc56c79602bd0ec6f0a40bfd0b138e939a71d22ead163ae9"},
                "dedup":{"id":"dedup-default",
                    "digest":"7bd8fd784129ffc11dc49a4fe706a90b459d8ca443d65113679dcd8ce08edffe"}},
            "decode_limits": super::support::value(&catalog,"policy")["aggregate_limits"]["decode"],
            "output_schema":catalog.0["output"].reference,
            "required_fidelity":["literal"],"admission_rules":["hold-partial"],
            "artifacts":[catalog.0["plugin"].reference],"platforms":[env::consts::OS],
            "qualification_state":"qualified",
            "qualification_evidence":[catalog.0["gold"].reference,catalog.0["threshold"].reference]
        });
        seal(&mut value, &mut catalog);
        profiles.push(value);
    }
    let safe = catalog.0["safe"].reference.clone();
    let registry = json!({
        "schema":"maestro-extraction-registry/2","id":"extraction","version":1,
        "collection_id":"garden","visibility":"public",
        "scope_tags":["workspace/default/collection/garden"],
        "owner_ref":catalog.0["owner"].reference,
        "profiles":profiles,"unknown_profile":safe,
        "qualification":catalog.0["qualification"].reference
    });
    let reference = put(&mut catalog, "extraction", &registry);
    let members = ["safe", "markdown", "novel"]
        .iter()
        .map(|id| catalog.0[*id].reference.clone())
        .collect();
    catalog
        .0
        .get_mut("extraction")
        .unwrap()
        .admission
        .references = members;
    let mut policy = super::support::value(&catalog, "policy");
    policy["sources"][0]["selected_profiles"] = json!([
        catalog.0["markdown"].reference,
        catalog.0["novel"].reference
    ]);
    put(&mut catalog, "policy", &policy);
    rebind(&mut collection, &mut catalog);
    (collection, catalog, reference)
}

/// Seal a definition with the approved versioned typed preimage.
pub(super) fn seal(value: &mut Value, catalog: &mut Catalog) {
    let profile: Profile = serde_json::from_value(value.clone()).unwrap();
    let bytes = definition_bytes(&profile.definition).unwrap();
    let digest = Digest::of(&bytes);
    value["definition_digest"] = json!(digest);
    let id = value["id"].as_str().unwrap();
    put(catalog, id, &json!({"synthetic":id}));
    let resource = catalog.0.get_mut(id).unwrap();
    resource.reference.digest = digest.clone();
    resource.bytes = bytes;
    resource.admission.digest = digest;
}

/// Re-pin a changed registry after a test mutates one profile.
pub(super) fn update(collection: &mut Value, catalog: &mut Catalog, value: &Value) -> Ref {
    let reference = put(catalog, "extraction", value);
    let members = ["safe", "markdown", "novel"]
        .iter()
        .map(|id| catalog.0[*id].reference.clone())
        .collect();
    catalog
        .0
        .get_mut("extraction")
        .unwrap()
        .admission
        .references = members;
    rebind(collection, catalog);
    reference
}

/// Bind the exact same bytes through the production direct-files adapter.
pub(super) fn direct(catalog: &Catalog) -> (PathBuf, DirectFiles) {
    let directory = scratch_directory().unwrap();
    let mut files = BTreeMap::new();
    for (id, resource) in &catalog.0 {
        let path = directory.join(id);
        fs::write(&path, &resource.bytes).unwrap();
        files.insert(
            id.clone(),
            LocalResource {
                path,
                admission: resource.admission.clone(),
            },
        );
    }
    (directory, DirectFiles::new(files))
}

/// One atomic synthetic block observation.
pub(super) fn structure() -> Observation {
    Observation::Block {
        id: "heading".into(),
    }
}

/// Remove qualification without changing any immutable wire bytes.
pub(super) fn unqualify(catalog: &mut Catalog, id: &str) {
    catalog.0.get_mut(id).unwrap().admission.status = AdmissionStatus::Held;
}

/// Resolve the existing source policy using its real validator.
pub(super) fn policy(collection: &Value, catalog: &Catalog) -> maestro_acquisition::CheckedPolicy {
    maestro_acquisition::validate(
        catalog,
        &serde_json::from_value(collection.clone()).unwrap(),
        &principal(&scopes()),
    )
    .unwrap()
}
