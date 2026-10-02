//! Read-only inspection and bounded pre-start inventory regression checks.
use super::{
    command::new_receipt,
    flow_edges::{Fixture, clean},
    inspect::inspect,
    output::Report,
};
use maestro_kernel::{
    acquisition::{Frontier, Handle, LeaseRequest, NewItem, Receipts, Status},
    artifact::Digest,
};
use std::time::{Duration, SystemTime};

#[test]
fn n14_inspect_pending_and_refuse_substituted_or_invalid_summary() {
    let fixture = Fixture::new(clean);
    let scope = "workspace/default/collection/garden".parse().unwrap();
    let inputs = fixture
        .db
        .retain(&scope, b"synthetic frozen inputs", &[])
        .unwrap();
    for case in [
        "pending", "valid", "run", "receipt", "status", "schema", "invalid",
    ] {
        let mut report = Report::new();
        report.run = Some(Handle::new());
        report.receipt = Some(Handle::new());
        report.status = Status::Complete;
        let mut receipt = new_receipt(&report, inputs).unwrap();
        fixture.db.begin(&scope, &receipt).unwrap();
        if case == "pending" {
            let result = inspect(&fixture.db, "reader", report.receipt.unwrap()).unwrap();
            assert_eq!(result.status, Status::Pending);
            assert_eq!(
                result.pending.first().unwrap().reason,
                "attempt_not_finalized"
            );
            continue;
        }
        match case {
            "run" => report.run = Some(Handle::new()),
            "receipt" => report.receipt = Some(Handle::new()),
            "status" => report.status = Status::Partial,
            "schema" => report.schema = "unsupported/0".into(),
            _ => (),
        }
        let bytes = if case == "invalid" {
            b"not JSON".to_vec()
        } else {
            serde_json::to_vec(&report).unwrap()
        };
        let summary = fixture.db.retain(&scope, &bytes, &[]).unwrap();
        receipt.status = Status::Complete;
        receipt.downstream = vec![summary];
        fixture.db.finish(&receipt).unwrap();
        let result = inspect(&fixture.db, "reader", receipt.attempt);
        assert_eq!(result.is_ok(), case == "valid", "{case}");
        if case == "valid" {
            assert_eq!(
                Receipts::page(&fixture.db, "reader", receipt.run, None, 1000)
                    .unwrap()
                    .last()
                    .unwrap()
                    .receipt,
                receipt.attempt
            );
            assert!(inspect(&fixture.db, "stranger", receipt.attempt).is_err());
        }
    }
    fixture.finish();
}

#[test]
fn n14_unknown_media_retains_raw_capture_but_holds_discovery() {
    let mut fixture = Fixture::new(clean);
    fixture.site.unknown_media = true;
    let report = fixture.sync();
    assert_eq!(report.status, Status::Partial);
    assert_eq!(report.completed.len(), 1);
    assert!(
        report
            .pending
            .iter()
            .any(|entry| entry.reason == "discovery_media_unknown")
    );
    fixture.finish();
}

#[test]
fn n14_held_source_retains_unique_pending_attempt_without_zero_start_claim() {
    let fixture = Fixture::new(clean);
    let now = SystemTime::now();
    let scope = "workspace/default/collection/garden".parse().unwrap();
    let writer = fixture
        .db
        .lease_source(
            "notes",
            &scope,
            LeaseRequest {
                holder: "earlier",
                now,
                term: Duration::from_secs(60),
            },
        )
        .unwrap();
    let report = fixture.sync();
    assert_eq!(report.status, Status::Blocked);
    assert_eq!(
        report.pending.first().unwrap().reason,
        "source_lease_unavailable"
    );
    let durable = inspect(&fixture.db, "reader", report.receipt.unwrap()).unwrap();
    assert_eq!(durable.run, report.run);
    assert_eq!(durable.status, Status::Pending);
    assert!(fixture.site.requests.lock().unwrap().is_empty());
    fixture.db.release_source(&writer, now).unwrap();
    fixture.finish();
}

#[test]
fn n14_reconciliation_cannot_erase_an_unacknowledged_item() {
    use maestro_kernel::acquisition::{
        InventoryPage, InventorySchema, ItemDisposition, Stage, StageItem,
    };
    let fixture = Fixture::new(clean);
    let scope = "workspace/default/collection/garden".parse().unwrap();
    let now = SystemTime::now();
    let writer = fixture
        .db
        .lease_source(
            "notes",
            &scope,
            LeaseRequest {
                holder: "earlier",
                now,
                term: Duration::from_secs(60),
            },
        )
        .unwrap();
    let item = fixture
        .db
        .enqueue(
            &writer,
            &NewItem {
                fetch_identity: "https://garden.example/docs/start".into(),
                authorization_context: Digest::of(b"reader"),
                representation_profile: fixture
                    .policy
                    .policy()
                    .sources
                    .first()
                    .unwrap()
                    .acquisition_profile
                    .digest
                    .clone(),
            },
            now,
        )
        .unwrap();
    let inputs = fixture
        .db
        .retain(&scope, b"synthetic frozen inputs", &[])
        .unwrap();
    let partition = fixture
        .db
        .retain(&scope, b"synthetic capture partition", &[])
        .unwrap();
    let mut report = Report::new();
    report.run = Some(Handle::new());
    report.receipt = Some(Handle::new());
    report.status = Status::Complete;
    let mut receipt = new_receipt(&report, inputs).unwrap();
    fixture.db.begin(&scope, &receipt).unwrap();
    receipt.status = Status::Complete;
    let page = InventoryPage {
        schema: InventorySchema::V1,
        partition,
        stage: Stage::Capture,
        complete: false,
        items: vec![StageItem {
            item: item.id.to_string().parse().unwrap(),
            disposition: ItemDisposition::Pending,
            evidence: None,
        }],
    };
    super::command::finish(
        &fixture.db,
        &scope,
        &mut receipt,
        (&[page], &[item]),
        &mut report,
    )
    .unwrap();
    assert_eq!(report.status, Status::Partial);
    assert_eq!(
        report.pending.first().unwrap().reason,
        "capture_reconciliation_pending"
    );
    assert_eq!(
        inspect(&fixture.db, "reader", report.receipt.unwrap())
            .unwrap()
            .status,
        Status::Partial
    );
    fixture.db.release_source(&writer, now).unwrap();
    fixture.finish();
}

#[test]
fn n14_seed_page_budget_dispatches_exactly_one_seed() {
    let fixture = Fixture::new(|policy| {
        policy["sources"][0]["seeds"] = serde_json::json!([
            "https://garden.example/docs/allowed",
            "https://garden.example/docs/final"
        ]);
        policy["sources"][0]["limits"]["pages"] = serde_json::json!(1);
    });
    let report = fixture.sync();
    assert_eq!(report.status, Status::Partial);
    assert_eq!(report.completed.len(), 1);
    assert!(
        report
            .pending
            .iter()
            .any(|entry| entry.reason == "page_budget")
    );
    assert_eq!(
        fixture
            .site
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|path| matches!(path.as_str(), "/docs/allowed" | "/docs/final"))
            .count(),
        1
    );
    fixture.finish();
}

#[test]
fn n14_zero_slot_leaf_with_only_scope_exclusions_is_complete() {
    let mut fixture = Fixture::new(|policy| {
        clean(policy);
        policy["sources"][0]["limits"]["pages"] = serde_json::json!(3);
    });
    fixture.site.leaf = super::flow_tests::Leaf::Linked;
    let report = fixture.sync();
    assert_eq!(report.status, Status::Complete);
    assert_eq!(report.overflow, 0);
    assert!(
        report
            .discarded
            .iter()
            .any(|entry| entry.reason == "beyond_declared_depth")
    );
    assert!(
        !fixture
            .site
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|path| path == "/docs/beyond")
    );
    fixture.finish();
}

#[test]
fn n14_changed_pending_context_is_held_without_fetching_it() {
    for account in [true, false] {
        let fixture = Fixture::new(clean);
        let scope = "workspace/default/collection/garden".parse().unwrap();
        let now = SystemTime::now();
        let writer = fixture
            .db
            .lease_source(
                "notes",
                &scope,
                LeaseRequest {
                    holder: "earlier",
                    now,
                    term: Duration::from_secs(60),
                },
            )
            .unwrap();
        let current_profile = fixture
            .policy
            .policy()
            .sources
            .first()
            .unwrap()
            .acquisition_profile
            .digest
            .clone();
        fixture
            .db
            .enqueue(
                &writer,
                &NewItem {
                    fetch_identity: "https://garden.example/docs/allowed".into(),
                    authorization_context: Digest::of(if account {
                        b"old account".as_slice()
                    } else {
                        b"reader".as_slice()
                    }),
                    representation_profile: if account {
                        current_profile
                    } else {
                        Digest::of(b"old profile")
                    },
                },
                now,
            )
            .unwrap();
        fixture.db.release_source(&writer, now).unwrap();
        let report = fixture.sync();
        assert_eq!(report.status, Status::Partial);
        assert!(
            report
                .pending
                .iter()
                .any(|entry| entry.reason == "context_changed")
        );
        assert_eq!(
            fixture
                .site
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|path| *path == "/docs/allowed")
                .count(),
            1
        );
        fixture.finish();
    }
}

#[test]
fn n14_nonpublic_auth_role_and_seed_ceiling_refuse_before_transport() {
    use super::{command::resolve, flow_tests::fixture_values};
    use maestro_acquisition::Principal;
    use std::env;
    let fixture = Fixture::new(clean);
    let scopes = fixture.db.visible("reader").unwrap();
    let principal = Principal {
        id: "reader",
        platform: env::consts::OS,
        scopes: &scopes,
    };
    for case in ["private", "auth", "seeds", "browser"] {
        let (collection, mut files) = fixture_values(|id, value| {
            if case == "private" && value.get("visibility").is_some() {
                value["visibility"] = serde_json::json!("private");
            }
            if case == "browser" && id == "http" {
                value["transport"] = serde_json::json!("browser_request");
            }
            match (case, id) {
                ("auth", "policy") => {
                    value["sources"][0]["auth_role"] = serde_json::json!("synthetic-account");
                }
                ("seeds", "policy") => {
                    value["sources"][0]["seeds"] = serde_json::json!(
                        (0..1001)
                            .map(|index| format!("https://garden.example/docs/legacy-{index}"))
                            .collect::<Vec<_>>()
                    );
                }
                _ => (),
            }
        });
        if case == "browser" {
            files
                .0
                .get_mut("adapter")
                .unwrap()
                .admission
                .capabilities
                .push("browser_request".into());
        }
        let result = resolve(&files, &collection, &principal);
        assert!(result.is_err(), "{case}");
        let error = result.unwrap_err().to_string();
        assert_eq!(
            error,
            match case {
                "private" => "public acquisition requires a public policy",
                "auth" | "browser" => "public manual acquisition capability unsupported",
                _ => "manual seed inventory exceeds the bounded MVP",
            },
            "{case}"
        );
    }
    assert!(fixture.site.requests.lock().unwrap().is_empty());
    fixture.finish();
}

#[test]
fn n14_depth_preserves_definitive_exclusions_and_out_of_scope_unknowns_do_not_hold() {
    use super::flow_tests::Leaf;
    let mut declared = Fixture::new(clean);
    declared.site.leaf = Leaf::Mixed;
    let report = declared.sync();
    assert_eq!(report.status, Status::Complete);
    for reason in ["policy_denial", "non_fetch_scheme", "beyond_declared_depth"] {
        assert!(
            report.discarded.iter().any(|entry| entry.reason == reason),
            "{report:?}"
        );
    }
    assert!(report.pending.is_empty());
    declared.finish();
    let mut run = Fixture::new(|policy| {
        clean(policy);
        policy["sources"][0]["discovery"][0]["depth"] = serde_json::json!(2);
    });
    run.site.leaf = Leaf::Mixed;
    let report = run.sync();
    assert_eq!(report.status, Status::Partial);
    for reason in ["unresolved_identity", "run_depth_limit"] {
        assert!(
            report.pending.iter().any(|entry| entry.reason == reason),
            "{report:?}"
        );
    }
    run.finish();
}

#[test]
fn n14_exhausted_fetch_slots_do_not_shrink_discovery_inventory() {
    let fixture = Fixture::new(|policy| {
        clean(policy);
        policy["sources"][0]["limits"]["pages"] = serde_json::json!(1);
    });
    let report = fixture.sync();
    assert_eq!(report.status, Status::Partial);
    assert_eq!(report.overflow, 1);
    let scopes = fixture.db.visible("reader").unwrap();
    assert_eq!(
        Frontier::page(&fixture.db, &scopes, "notes", None, 1000)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(fixture.site.requests.lock().unwrap().len(), 2);
    fixture.finish();
}

#[test]
fn n14_shared_controls_refuse_authenticated_source_before_authority() {
    use super::{
        controls::Controls,
        controls::request,
        flow_tests::{Grants, fixture_with},
    };
    use maestro_acquisition::{
        Principal,
        policy::{decision::AdmissionControls, resolve::validate},
    };
    use std::env;
    let fixture = Fixture::new(clean);
    let scopes = fixture.db.visible("reader").unwrap();
    let principal = Principal {
        id: "reader",
        platform: env::consts::OS,
        scopes: &scopes,
    };
    let (collection, files) = fixture_with(|policy| {
        policy["sources"][0]["auth_role"] = serde_json::json!("synthetic-account");
    });
    let policy = validate(&files, &collection, &principal).unwrap();
    let grants = Grants::default();
    let controls = Controls::new(
        &policy,
        &grants,
        "reader",
        "workspace/default/collection/garden",
    );
    let source = policy.policy().sources.first().unwrap();
    assert!(
        controls
            .caller(
                source,
                &request(
                    "notes",
                    "https://garden.example/docs/start",
                    "2026-10-02T00:00:00Z"
                )
            )
            .is_err()
    );
    fixture.finish();
}
