//! Frozen typed preimages: declaration order must not depend on JSON map features.
use super::{
    manifest::{Activation, Change, Proposal},
    recovery::Recovery,
    storage::encode,
};
use crate::{
    Ref,
    policy::{
        limits::{DecodeLimits, Limits, XmlEntities},
        manifest::{AcquisitionManifest, AdaptationPolicy, BaselineKind, ManifestSchema},
        resource::{Resource, Visibility},
        schema::{PolicySchema, SourcePolicy},
    },
};
use maestro_kernel::{
    acquisition::{Handle, Progress, Reason, Status},
    artifact::Digest,
};
use serde::Serialize;
use std::str::from_utf8;

fn reference(id: &str) -> Ref {
    Ref {
        id: id.into(),
        digest: Digest::parse("1111111111111111111111111111111111111111111111111111111111111111")
            .unwrap(),
    }
}
fn resource<S>(schema: S, id: &str) -> Resource<S> {
    Resource {
        schema,
        id: id.into(),
        version: 1.try_into().unwrap(),
        collection_id: "garden".into(),
        visibility: Visibility::Public,
        scope_tags: vec!["workspace/default/collection/garden".into()],
        owner_ref: Ref {
            id: "owner".into(),
            digest: Digest::parse(
                "2222222222222222222222222222222222222222222222222222222222222222",
            )
            .unwrap(),
        },
    }
}
fn vector(value: &impl Serialize, expected: &str, digest: &str) {
    let bytes = encode(value).unwrap();
    assert_eq!(from_utf8(&bytes).unwrap(), expected.trim());
    assert_eq!(Digest::of(&bytes).as_str(), digest);
}
#[test]
fn n30_review_canonical_encoder_typed_manifest() {
    let manifest = AcquisitionManifest {
        resource: resource(ManifestSchema::V2, "manifest"),
        baseline: reference("policy"),
        processing_baseline: reference("initial-snapshot"),
        baseline_kind: BaselineKind::Catalog,
        proposals: vec![],
        activations: vec![],
        active: reference("policy"),
        effective_digest: Digest::parse(
            "3333333333333333333333333333333333333333333333333333333333333333",
        )
        .unwrap(),
    };
    vector(
        &manifest,
        include_str!("golden-manifest.json"),
        "8119f47da866dab1373a440054a3b611ce209f016703d186c4b361f3b47601d0",
    );
}
#[test]
fn n30_typed_overlay_and_recovery_golden_vectors() {
    let handle: Handle = "00000000000000000000000001".parse().unwrap();
    let proposal = Proposal {
        expected_baseline: reference("policy"),
        expected_active: reference("policy"),
        changes: vec![Change::SetCleanup {
            rules: reference("rules"),
        }],
        candidate: reference("candidate").digest,
        evidence: vec![handle],
        report: handle,
        rollback: reference("policy"),
    };
    let activation = Activation {
        proposal: reference("proposal"),
        gate: handle,
        previous: reference("policy"),
        restores: Some(reference("policy")),
    };
    let proposal_marker = Recovery::new(
        reference("old").digest,
        reference("new").digest,
        reference("policy"),
        None,
    );
    let activation_marker = Recovery::new(
        reference("old").digest,
        reference("new").digest,
        reference("policy"),
        Some(Progress {
            receipt: handle,
            status: Status::Complete,
            reason: Reason::None,
        }),
    );
    vector(
        &proposal,
        include_str!("golden-proposal.json"),
        "54320e0e61b8675de226ec3fcf6f693df3f530a6ccb7fabcb918b38f40964e46",
    );
    vector(
        &activation,
        include_str!("golden-activation.json"),
        "dea80aa995be34bd3ec658db10b103efd0a58cad8d8094cfc78c5f79055bbc84",
    );
    vector(
        &proposal_marker,
        include_str!("golden-proposal-recovery.json"),
        "6699b2388b73db17656b0a26d86279c2109df4d29e73554dc2545817e7cc7bd0",
    );
    vector(
        &activation_marker,
        include_str!("golden-activation-recovery.json"),
        "96acf7259c8cac19230160b071041c768ae6ed4c256dd1e0182fdebc75247990",
    );
}

fn limits() -> Limits {
    let one = 1.try_into().unwrap();
    Limits {
        requests: one,
        pages: one,
        partitions: one,
        redirects: one,
        depth: one,
        elapsed_ms: one,
        wire_bytes: one,
        dom_bytes: one,
        asset_bytes: one,
        staging_bytes: one,
        cpu_millicores: one,
        memory_bytes: one,
        source_runs: one,
        origin_concurrency: one,
        free_reserve_bytes: one,
        gpu_reserve_bytes: one,
        origin_interval_ms: one,
        retries: 0,
        max_backoff_ms: 0,
        gpu_batches: 0,
        gpu_bytes: 0,
        decode: DecodeLimits {
            expanded_bytes: one,
            expansion_ratio: one,
            nested_levels: one,
            members: one,
            decoded_pixels: one,
            elapsed_ms: one,
            memory_bytes: one,
            xml_entities: XmlEntities::Disabled,
        },
    }
}
#[test]
fn n30_typed_effective_digest_tuple_golden_vector() {
    let one = 1.try_into().unwrap();
    // This encoding vector exercises the exact digest tuple, not policy admission.
    let policy = SourcePolicy {
        resource: resource(PolicySchema::V1, "policy"),
        sources: vec![],
        registries: vec![],
        profiles: reference("profiles"),
        acquisition_profiles: vec![reference("acquisition")],
        address_table: reference("addresses"),
        aggregate_limits: limits(),
        adaptation: AdaptationPolicy {
            matrix: reference("matrix"),
            thresholds: reference("thresholds"),
            baseline: reference("baseline"),
            minimum_sample: one,
            consecutive_runs: one,
            activation_interval_ms: one,
            automatic_classes: vec![],
        },
        retention_rule: reference("retention"),
        qualification: reference("qualification"),
    };
    vector(
        &super::writer::EffectivePreimage {
            schema: "maestro-acquisition-effective/2",
            policy: &policy,
            processing_baseline: &reference("initial-snapshot"),
            activations: &[reference("activation")],
        },
        include_str!("golden-effective.json"),
        "66ba959616ab9892bc827669d28f72a96cd0fb993691d4c35b4adfd80dd11db5",
    );
}
