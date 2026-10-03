//! Corrupt, pending and incompatible dependencies cannot be reused as completed.
use super::{
    n12_parents::derive,
    n12_support::Fixture,
    n37_support::{item, scope},
};
use maestro_kernel::acquisition::{Captures, Frontier, LeaseRequest, Representation};
use rusqlite::Connection;
use std::time::Duration;

#[test]
fn n37_unacknowledged_prerequisite_is_not_a_completed_stage() {
    let mut fixture = Fixture::new();
    let parent = fixture.prepare().unwrap();
    derive(&mut fixture, parent, Representation::SelectedHtml);
    assert!(
        fixture.prepare().is_err(),
        "prepared prerequisite promoted before acknowledgement"
    );
}

#[test]
fn n37_dependency_verification_checks_current_scope_and_source() {
    for changed in ["scope", "source"] {
        let mut fixture = Fixture::new();
        let parent_item = fixture.context.item.item;
        let parent = fixture.prepare().unwrap();
        fixture
            .db
            .acknowledge_capture(&fixture.context, parent)
            .unwrap();
        derive(&mut fixture, parent, Representation::SelectedHtml);
        let child = fixture.prepare().unwrap();
        fixture
            .db
            .acknowledge_capture(&fixture.context, child)
            .unwrap();
        let sql = Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
        if changed == "scope" {
            sql.execute(
                "UPDATE acquisition_evidence
                 SET scope = 'workspace/default/collection/other' WHERE id = ?1",
                [parent.to_string()],
            )
            .unwrap();
        } else {
            fixture
                .db
                .lease_source(
                    "other",
                    &scope(),
                    LeaseRequest {
                        holder: "other",
                        now: fixture.context.now,
                        term: Duration::from_secs(30),
                    },
                )
                .unwrap();
            sql.execute(
                "UPDATE acquisition_frontier SET source = 'other' WHERE id = ?1",
                [parent_item.to_string()],
            )
            .unwrap();
        }
        assert_eq!(
            fixture.db.capture_for(&scope(), &item(&fixture)).unwrap(),
            None,
            "changed parent {changed} reused"
        );
        assert!(fixture.db.read_capture(&fixture.context, child, 4).is_err());
    }
}

#[test]
fn n37_dependency_representation_matrix_remains_closed() {
    let mut fixture = Fixture::new();
    let parent = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, parent)
        .unwrap();
    derive(&mut fixture, parent, Representation::ApiRecord);
    let api = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, api)
        .unwrap();
    derive(&mut fixture, api, Representation::SelectedHtml);
    assert!(
        fixture.prepare().is_err(),
        "undeclared API-to-HTML derivation reused"
    );
}

#[test]
fn n37_capture_dependency_cycle_refuses_without_recursive_stack_growth() {
    let mut fixture = Fixture::new();
    let parent = fixture.prepare().unwrap();
    // Model corrupted kernel metadata while retaining intact artifact hashes.
    fixture.envelope.parent = Some(parent);
    fixture.envelope.representation = Representation::SelectedHtml;
    let digest = fixture
        .db
        .put(
            &serde_json::to_vec(&fixture.envelope).unwrap(),
            "application/json",
        )
        .unwrap();
    let sql = Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    sql.execute(
        "UPDATE acquisition_evidence SET artifact = ?2 WHERE id = ?1",
        [parent.to_string(), digest.as_str().to_owned()],
    )
    .unwrap();
    sql.execute(
        "UPDATE acquisition_frontier SET capture = ?2 WHERE id = ?1",
        [
            fixture.context.item.item.to_string(),
            digest.as_str().to_owned(),
        ],
    )
    .unwrap();
    let row = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        1,
    )
    .unwrap()
    .remove(0);
    assert!(fixture.db.verify_capture(&scope(), &row, parent).is_err());
}

#[test]
fn n37_missing_initial_inventory_refuses_before_source_release() {
    use super::n37_support::receipt;
    use maestro_acquisition::capture::{Cancellation, cancel_run};
    use maestro_kernel::acquisition::{
        Handle, InventoryPage, InventorySchema, ItemDisposition, Receipts, Stage, StageItem, Status,
    };
    let mut fixture = Fixture::new();
    let mut existing = receipt(&mut fixture);
    existing.attempt = Handle::new();
    existing.run = Handle::new();
    let initial = InventoryPage {
        schema: InventorySchema::V1,
        partition: existing.inputs,
        stage: Stage::Discovery,
        complete: true,
        items: vec![StageItem {
            item: fixture.envelope.item,
            disposition: ItemDisposition::Discovered,
            evidence: None,
        }],
    };
    existing
        .inventories
        .push(fixture.db.retain_inventory(&scope(), &initial).unwrap());
    fixture.db.begin(&scope(), &existing).unwrap();
    fixture
        .db
        .release_source(&fixture.context.writer, fixture.context.now)
        .unwrap();
    let holder = existing.attempt.to_string();
    fixture.context.writer = fixture
        .db
        .lease_source(
            "notes",
            &scope(),
            LeaseRequest {
                holder: &holder,
                now: fixture.context.now,
                term: Duration::from_secs(30),
            },
        )
        .unwrap();
    let frontier = vec![item(&fixture)];
    let pages = vec![InventoryPage {
        schema: InventorySchema::V1,
        partition: existing.inputs,
        stage: Stage::Capture,
        complete: false,
        items: vec![StageItem {
            item: fixture.envelope.item,
            disposition: ItemDisposition::Pending,
            evidence: None,
        }],
    }];
    let cancellation = Cancellation {
        writer: &fixture.context.writer,
        now: fixture.context.now,
        scope: &scope(),
        receipt: &existing,
        pages: &pages,
        frontier: &frontier,
    };
    assert!(cancel_run(&fixture.db, &cancellation).is_err());
    assert_eq!(
        fixture
            .db
            .inspect("reader", existing.attempt)
            .unwrap()
            .unwrap()
            .status,
        Status::Pending
    );
    assert!(
        fixture
            .db
            .lease_source(
                "notes",
                &scope(),
                LeaseRequest {
                    holder: "contender",
                    now: fixture.context.now,
                    term: Duration::from_secs(30)
                }
            )
            .is_err(),
        "omitted initial stage released the source before finish rejected it"
    );
}

#[test]
fn n37_stale_cancel_cannot_finalize_pending_receipt() {
    use super::n37_support::receipt;
    use maestro_acquisition::capture::{Cancellation, cancel_run};
    use maestro_kernel::acquisition::{
        InventoryPage, InventorySchema, ItemDisposition, Receipts, Stage, StageItem, Status,
    };
    let mut fixture = Fixture::new();
    let existing = receipt(&mut fixture);
    let mut stale = fixture.context.writer.clone();
    stale.epoch += 1;
    let frontier = vec![item(&fixture)];
    let pages = vec![InventoryPage {
        schema: InventorySchema::V1,
        partition: existing.inputs,
        stage: Stage::Capture,
        complete: false,
        items: vec![StageItem {
            item: fixture.envelope.item,
            disposition: ItemDisposition::Pending,
            evidence: None,
        }],
    }];
    let cancellation = Cancellation {
        writer: &stale,
        now: fixture.context.now,
        scope: &scope(),
        receipt: &existing,
        pages: &pages,
        frontier: &frontier,
    };
    assert!(cancel_run(&fixture.db, &cancellation).is_err());
    assert_eq!(
        fixture
            .db
            .inspect("reader", existing.attempt)
            .unwrap()
            .unwrap()
            .status,
        Status::Pending,
        "stale fence finalized the receipt before refusing release"
    );
}

#[test]
fn n37_retained_redirect_and_final_reapply_authority_and_policy_before_adoption_or_reuse() {
    use super::{
        n07_parse_url_identity_and_denial_precedence::Controls,
        n09_support::{Grants, policy_with},
        n37_support::current,
    };
    use maestro_acquisition::lifecycle::resume::{completed, prepared};
    use maestro_kernel::acquisition::{RedirectHop, SafeIdentity};
    for redirect in [false, true] {
        let mut fixture = Fixture::new();
        let hop = SafeIdentity::new("https://garden.example/docs/hop").unwrap();
        fixture.envelope.redirects = vec![RedirectHop {
            identity: if redirect {
                hop.clone()
            } else {
                fixture.envelope.requested.clone()
            },
            status: 302,
        }];
        if !redirect {
            fixture.envelope.final_identity = hop;
        }
        let capture = fixture.prepare().unwrap();
        let controls = Controls::default();
        let grants = Grants::default();
        let access = current(&fixture, &fixture.policy, &controls, &grants);
        let row = item(&fixture);
        assert!(prepared(&fixture.db, &access, (&row, &fixture.context), capture, 4).is_ok());
        fixture
            .db
            .acknowledge_capture(&fixture.context, capture)
            .unwrap();
        let row = item(&fixture);
        assert_eq!(
            completed(&fixture.db, &access, &row).unwrap(),
            Some(capture)
        );
        // Initial request and (for final denial) the original redirect still have authority.
        grants.calls.set(0);
        grants.refuse_after.set(if redirect { 1 } else { 2 });
        assert!(
            completed(&fixture.db, &access, &row).is_err(),
            "revoked retained hop reused"
        );
        grants.calls.set(0);
        assert!(
            prepared(&fixture.db, &access, (&row, &fixture.context), capture, 4).is_err(),
            "revoked retained hop adopted"
        );
        grants.refuse_after.set(0);
        let tightened = policy_with(|value| {
            for selector in value["sources"][0]["selectors"].as_array_mut().unwrap() {
                selector["path_prefix"] = "/docs/start".into();
            }
        });
        let access = current(&fixture, &tightened, &controls, &grants);
        assert!(
            completed(&fixture.db, &access, &row).is_err(),
            "tightened retained hop reused"
        );
        assert!(
            prepared(&fixture.db, &access, (&row, &fixture.context), capture, 4).is_err(),
            "tightened retained hop adopted"
        );
    }
}

#[test]
fn n37_redacted_redirect_or_final_cannot_prove_current_admission() {
    use super::{
        n07_parse_url_identity_and_denial_precedence::Controls,
        n09_support::{Grants, policy_with},
        n37_support::current,
    };
    use maestro_acquisition::lifecycle::resume::{completed, prepared};
    use maestro_kernel::acquisition::{RedirectHop, SafeIdentity};
    for redirect in [false, true] {
        let policy = policy_with(|value| {
            value["sources"][0]["identity"]["meaningful_queries"] = serde_json::json!(["q"]);
        });
        let mut fixture = Fixture::with_policy(policy);
        let redacted = SafeIdentity::new("https://garden.example/docs/hop?q=synthetic").unwrap();
        assert!(redacted.unredacted().is_none());
        assert_eq!(
            fixture.envelope.requested.unredacted(),
            Some("https://garden.example/docs/start")
        );
        if redirect {
            fixture.envelope.redirects.push(RedirectHop {
                identity: redacted,
                status: 302,
            });
        } else {
            fixture.envelope.final_identity = redacted;
        }
        let capture = fixture.prepare().unwrap();
        let controls = Controls::default();
        let grants = Grants::default();
        let access = current(&fixture, &fixture.policy, &controls, &grants);
        let row = item(&fixture);
        assert!(
            prepared(&fixture.db, &access, (&row, &fixture.context), capture, 4).is_err(),
            "redacted hop adopted"
        );
        fixture
            .db
            .acknowledge_capture(&fixture.context, capture)
            .unwrap();
        assert!(
            completed(&fixture.db, &access, &item(&fixture)).is_err(),
            "redacted hop reused"
        );
    }
}

#[test]
fn n37_retained_requested_query_reconstructs_only_matching_provenance() {
    use super::{
        n07_parse_url_identity_and_denial_precedence::Controls,
        n09_support::{Grants, policy_with},
        n37_support::current,
    };
    use maestro_kernel::acquisition::SafeIdentity;
    let policy = policy_with(|value| {
        value["sources"][0]["identity"]["meaningful_queries"] = serde_json::json!(["q"]);
    });
    let fixture = Fixture::with_policy(policy);
    let mut row = item(&fixture);
    row.request.fetch_identity = "https://garden.example/docs/start?q=known".into();
    let mut envelope = fixture.envelope.clone();
    envelope.final_identity = SafeIdentity::new(&row.request.fetch_identity).unwrap();
    let controls = Controls::default();
    let grants = Grants::default();
    let access = current(&fixture, &fixture.policy, &controls, &grants);
    assert!(access.check(&row).is_ok());
    assert!(access.check_envelope(&row, &envelope).is_ok());
    envelope.final_identity =
        SafeIdentity::new("https://garden.example/docs/start?q=other").unwrap();
    assert!(
        access.check_envelope(&row, &envelope).is_err(),
        "different query inferred from the frontier request"
    );
}
