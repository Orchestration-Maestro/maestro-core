//! Individual safety guards and recovery identity proofs for N30.
use super::{
    n30_support::{Events, Fixture, Grant},
    support,
};
use maestro_acquisition::{
    Ref, Refusal,
    adaptation::{AtomicCommit, ConfigurationWriter, LocalWriter, WriteError},
};
use maestro_kernel::{
    acquisition::Receipts,
    artifact::Digest,
    scope::{Right, Scope},
};
use serde_json::{Value, json};
use std::fs;

#[test]
fn n30_uncommitted_initial_overlays_refuse() {
    for field in ["active", "proposals", "activations"] {
        let fixture = Fixture::new();
        let principal = support::principal(&fixture.scopes);
        let events = Events::default();
        let mut initial = fixture.manifest.clone();
        let other = Ref {
            id: "other".into(),
            digest: Digest::of(b"other"),
        };
        match field {
            "active" => initial.active = other,
            "proposals" => initial.proposals.push(other),
            _ => initial.activations.push(other),
        }
        assert!(
            LocalWriter::open(
                &fixture.root.join("overlay"),
                fixture.context(&principal, &events, &Grant, &AtomicCommit),
                &initial
            )
            .is_err()
        );
        assert!(!fixture.root.join("overlay/manifest.json").exists());
    }
}

#[test]
fn n30_exact_metadata_and_baseline_kind_are_frozen() {
    for field in [
        "collection_id",
        "visibility",
        "scope_tags",
        "owner_ref",
        "id",
        "baseline_kind",
    ] {
        let fixture = Fixture::new();
        let principal = support::principal(&fixture.scopes);
        let events = Events::default();
        let root = fixture.root.join("overlay");
        LocalWriter::open(
            &root,
            fixture.context(&principal, &events, &Grant, &AtomicCommit),
            &fixture.manifest,
        )
        .unwrap();
        let mut state: Value =
            serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
        match field {
            "collection_id" | "id" => state[field] = json!("other"),
            "visibility" => state[field] = json!("private"),
            "scope_tags" => state[field] = json!(["workspace/default/collection/other"]),
            "owner_ref" => state[field]["id"] = json!("other"),
            _ => state[field] = json!("local"),
        }
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec(&state).unwrap(),
        )
        .unwrap();
        assert!(
            LocalWriter::open(
                &root,
                fixture.context(&principal, &events, &Grant, &AtomicCommit),
                &fixture.manifest
            )
            .is_err(),
            "{field}"
        );
    }
}

#[test]
fn n30_effective_digest_and_active_pointer_are_verified() {
    for field in ["effective_digest", "active"] {
        let fixture = Fixture::new();
        let principal = support::principal(&fixture.scopes);
        let events = Events::default();
        let root = fixture.root.join("overlay");
        LocalWriter::open(
            &root,
            fixture.context(&principal, &events, &Grant, &AtomicCommit),
            &fixture.manifest,
        )
        .unwrap();
        let mut state: Value =
            serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
        if field == "active" {
            state[field]["digest"] = json!(Digest::of(b"bad active"));
        } else {
            state[field] = json!(Digest::of(b"bad digest"));
        }
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec(&state).unwrap(),
        )
        .unwrap();
        assert!(
            LocalWriter::open(
                &root,
                fixture.context(&principal, &events, &Grant, &AtomicCommit),
                &fixture.manifest
            )
            .is_err(),
            "{field}"
        );
    }
}

#[test]
fn n30_proposal_bindings_and_nonempty_inputs_refuse() {
    for field in ["baseline", "active", "rollback", "changes", "evidence"] {
        let fixture = Fixture::new();
        let principal = support::principal(&fixture.scopes);
        let events = Events::default();
        let writer = LocalWriter::open(
            &fixture.root.join("overlay"),
            fixture.context(&principal, &events, &Grant, &AtomicCommit),
            &fixture.manifest,
        )
        .unwrap();
        let mut proposal = fixture.proposal.clone();
        let other = Ref {
            id: "other".into(),
            digest: Digest::of(b"other"),
        };
        match field {
            "baseline" => proposal.expected_baseline = other,
            "active" => proposal.expected_active = other,
            "rollback" => proposal.rollback = other,
            "changes" => proposal.changes.clear(),
            _ => proposal.evidence.clear(),
        }
        assert!(
            writer
                .propose(
                    &fixture.manifest.baseline,
                    &fixture.manifest.active,
                    &proposal
                )
                .is_err(),
            "{field}"
        );
        assert!(writer.current().unwrap().proposals.is_empty());
    }
}

#[test]
fn n30_proposal_must_be_registered_and_digest_bound() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let root = fixture.root.join("overlay");
    let writer = LocalWriter::open(
        &root,
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    let proposal = writer
        .propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &fixture.proposal,
        )
        .unwrap();
    let mut state: Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    state["proposals"] = json!([]);
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec(&state).unwrap(),
    )
    .unwrap();
    assert_eq!(
        writer.activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
            fixture.gate
        ),
        Err(WriteError::Refused(Refusal::Missing))
    );
    state["proposals"] =
        json!([{ "id": proposal.id, "digest": Digest::of(b"wrong stored bytes") }]);
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec(&state).unwrap(),
    )
    .unwrap();
    assert_eq!(writer.current(), Err(WriteError::Refused(Refusal::Digest)));
}

#[test]
fn n30_stored_proposal_cannot_evade_expected_state() {
    for field in ["expected_baseline", "expected_active"] {
        let fixture = Fixture::new();
        let principal = support::principal(&fixture.scopes);
        let events = Events::default();
        let root = fixture.root.join("overlay");
        let writer = LocalWriter::open(
            &root,
            fixture.context(&principal, &events, &Grant, &AtomicCommit),
            &fixture.manifest,
        )
        .unwrap();
        let mut proposal = fixture.proposal.clone();
        if field == "expected_baseline" {
            proposal.expected_baseline.digest = Digest::of(b"other");
        } else {
            proposal.expected_active.digest = Digest::of(b"other");
        }
        let bytes = serde_json::to_vec(&proposal).unwrap();
        let scope: Scope = fixture.manifest.resource.scope_tags[0].parse().unwrap();
        let handle = fixture
            .db
            .retain(&scope, &bytes, &[proposal.report])
            .unwrap();
        let reference = Ref {
            id: handle.to_string(),
            digest: Digest::of(&bytes),
        };
        let mut state: Value =
            serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
        state["proposals"] = json!([reference]);
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec(&state).unwrap(),
        )
        .unwrap();
        assert_eq!(
            writer.activate(
                &fixture.manifest.baseline,
                &fixture.manifest.active,
                &reference,
                fixture.gate
            ),
            Err(WriteError::Conflict),
            "{field}"
        );
    }
}

#[test]
fn n30_report_evidence_recheck_current_transitive_grants() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let scope: Scope = "workspace/default/collection/private-synthetic"
        .parse()
        .unwrap();
    fixture
        .db
        .grant(principal.id, &scope, Right::Read, "owner")
        .unwrap();
    let private = fixture
        .db
        .retain(&scope, b"private synthetic canary", &[])
        .unwrap();
    let report_scope: Scope = fixture.manifest.resource.scope_tags[0].parse().unwrap();
    let report = fixture
        .db
        .retain(&report_scope, b"report", &[private])
        .unwrap();
    let mut proposal = fixture.proposal.clone();
    proposal.report = report;
    let writer = LocalWriter::open(
        &fixture.root.join("overlay"),
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    let reference = writer
        .propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal,
        )
        .unwrap();
    fixture
        .db
        .revoke(principal.id, &scope, Right::Read, "owner")
        .unwrap();
    assert_eq!(
        writer.activate(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &reference,
            fixture.gate
        ),
        Err(WriteError::Refused(Refusal::Access))
    );
    assert!(events.0.lock().unwrap().is_empty());
    assert!(
        fixture
            .db
            .read("unauthorized", reference.id.parse().unwrap())
            .unwrap()
            .is_none()
    );
}

#[test]
fn n30_proposal_report_is_required_before_pointer_exposure() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let writer = LocalWriter::open(
        &fixture.root.join("overlay"),
        fixture.context(&principal, &events, &Grant, &AtomicCommit),
        &fixture.manifest,
    )
    .unwrap();
    let mut proposal = fixture.proposal.clone();
    let denied_scope: Scope = "workspace/default/collection/denied-report"
        .parse()
        .unwrap();
    proposal.report = fixture
        .db
        .retain(&denied_scope, b"inaccessible report", &[])
        .unwrap();
    assert_eq!(
        writer.propose(
            &fixture.manifest.baseline,
            &fixture.manifest.active,
            &proposal
        ),
        Err(WriteError::Refused(Refusal::Access))
    );
    let raw: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("overlay/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(raw["proposals"], json!([]));
    assert!(writer.current().unwrap().proposals.is_empty());
}

#[test]
fn n30_invalid_initial_wire_shape_never_exposes_pointer() {
    let fixture = Fixture::new();
    let principal = support::principal(&fixture.scopes);
    let events = Events::default();
    let mut initial = fixture.manifest.clone();
    initial.resource.id = "../not-an-id".into();
    assert!(
        LocalWriter::open(
            &fixture.root.join("overlay"),
            fixture.context(&principal, &events, &Grant, &AtomicCommit),
            &initial
        )
        .is_err()
    );
    assert!(!fixture.root.join("overlay/manifest.json").exists());
}
