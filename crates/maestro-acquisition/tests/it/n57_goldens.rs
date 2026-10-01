//! Frozen typed preimages, separate from executable qualification evidence.
use super::{n32_support, n57_support};
use maestro_acquisition::{
    adaptation::{
        Proposal,
        artifacts::{CleanupRules, DedupKeys, S1ChunkStrategy},
        snapshot::{ProcessingSnapshot, SnapshotSchema},
    },
    extraction::outcome::ProfileSelection,
    policy::resolve::parse_resource,
};
use maestro_kernel::artifact::Digest;
use serde::Serialize;
use std::collections::BTreeMap;

fn vector(value: &impl Serialize, golden: &str, digest: &str) {
    let bytes = serde_json::to_vec(value).unwrap();
    assert_eq!(bytes, golden.lines().collect::<String>().as_bytes());
    assert_eq!(Digest::of(&bytes).as_str(), digest);
}
#[test]
fn n57_definition_and_selection_golden_vectors() {
    let cleanup: CleanupRules =
        parse_resource(include_bytes!("../fixtures/adaptation/cleanup.json")).unwrap();
    vector(
        &cleanup,
        include_str!("../fixtures/adaptation/cleanup.json"),
        "c2476119870a58d3d83e9b9ca7c5591b945d8347242206d727c478d9aba94e4b",
    );
    let chunk: S1ChunkStrategy =
        parse_resource(include_bytes!("../fixtures/adaptation/chunk.json")).unwrap();
    vector(
        &chunk,
        include_str!("../fixtures/adaptation/chunk.json"),
        "91fbe5e26ee549eabfa05a3105832ab2ede131d6c815f0f7e5c43647752434dd",
    );
    let dedup: DedupKeys =
        parse_resource(include_bytes!("../fixtures/adaptation/dedup.json")).unwrap();
    vector(
        &dedup,
        include_str!("../fixtures/adaptation/dedup.json"),
        "e495c2566841150247b8567c0d316bc112173ca6d850931941f3d2102bdde2d4",
    );
    let selected: ProfileSelection =
        parse_resource(include_bytes!("../fixtures/adaptation/selection.json")).unwrap();
    vector(
        &selected,
        include_str!("../fixtures/adaptation/selection.json"),
        "f18ce36f46ecba5de07c3a767af13aff5a5713a1e177aeaef69990a3e68e8e12",
    );
    let original = Digest::of(&serde_json::to_vec(&cleanup).unwrap());
    let mut changed = cleanup;
    changed.rules[0].reason_code = "other".into();
    assert_ne!(original, Digest::of(&serde_json::to_vec(&changed).unwrap()));
    let original = Digest::of(&serde_json::to_vec(&chunk).unwrap());
    let mut changed = chunk;
    changed.model.digest = Digest::of(b"other model");
    assert_ne!(original, Digest::of(&serde_json::to_vec(&changed).unwrap()));
    let original = Digest::of(&serde_json::to_vec(&dedup).unwrap());
    let mut changed = dedup;
    changed.exact.reverse();
    assert_ne!(original, Digest::of(&serde_json::to_vec(&changed).unwrap()));
}
#[test]
fn n57_snapshot_golden_and_btree_insertion_order() {
    let effective = n32_support::fixture();
    let snapshot = ProcessingSnapshot {
        resource: n57_support::resource(SnapshotSchema::V1, "golden-snapshot", &effective.policy),
        effective,
    };
    vector(
        &snapshot,
        include_str!("../fixtures/adaptation/snapshot.json"),
        "07bc9b9713c2e0bdebb406408f8762dfdce0454e269555dca183b79f5c75520f",
    );
    let mut reordered = snapshot.clone();
    reordered.effective.profiles = snapshot
        .effective
        .profiles
        .iter()
        .rev()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        serde_json::to_vec(&snapshot).unwrap(),
        serde_json::to_vec(&reordered).unwrap()
    );
    reordered
        .effective
        .sources
        .get_mut("notes")
        .unwrap()
        .2
        .cleanup
        .digest = Digest::of(b"different default");
    assert_ne!(
        Digest::of(&serde_json::to_vec(&snapshot).unwrap()),
        Digest::of(&serde_json::to_vec(&reordered).unwrap())
    );
}
#[test]
fn n57_owning_schemas_include_kernel_limits() {
    let schema = serde_json::to_value(schemars::schema_for!(S1ChunkStrategy)).unwrap();
    assert!(schema["$defs"].get("Limits").is_some());
    assert_eq!(schema["$defs"]["Limits"]["additionalProperties"], false);
    let schema = serde_json::to_value(schemars::schema_for!(ProcessingSnapshot)).unwrap();
    assert!(schema["$defs"].get("EffectiveConfiguration").is_some());
}

#[test]
fn n57_profile_change_and_rollback_bind_typed_snapshot_vectors() {
    let proposal: Proposal =
        parse_resource(include_bytes!("../fixtures/adaptation/profile-change.json")).unwrap();
    vector(
        &proposal,
        include_str!("../fixtures/adaptation/profile-change.json"),
        "97cae486987f8b0376c0183289c1ca2fbb00284cb69b309cd96b07f73ec72258",
    );
    let rollback: Proposal =
        parse_resource(include_bytes!("../fixtures/adaptation/rollback.json")).unwrap();
    vector(
        &rollback,
        include_str!("../fixtures/adaptation/rollback.json"),
        "be1eb5d2a8f37a7c1ac621892a2e388f5d336c1657603a936585b8cb35f59145",
    );
    let snapshot: ProcessingSnapshot =
        parse_resource(include_bytes!("../fixtures/adaptation/snapshot.json")).unwrap();
    assert_eq!(
        proposal.candidate,
        Digest::of(&serde_json::to_vec(&snapshot).unwrap())
    );
    assert_eq!(rollback.candidate, proposal.candidate);
}
