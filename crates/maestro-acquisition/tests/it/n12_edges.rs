//! Adjacent denied capture boundaries and real scoped-store failure cases.
use super::{
    n11_support,
    n12_support::{Fixture, response},
};
use maestro_acquisition::{
    capture::{
        CaptureBudget, Captures, RedirectHop, Representation, SafeHeader, SafeIdentity, Transport,
        http_envelope, prepare,
    },
    transport::budget::Usage,
};
use maestro_kernel::{
    acquisition::{
        DispatchRequest, Frontier, Handle, LeaseRequest, NewItem, ReceiptError, Receipts,
        safe_header_names,
    },
    artifact::Digest,
};
use std::{
    fs,
    time::{Duration, Instant},
};

#[test]
fn n12_schema_source_item_status_media_and_redirect_bounds_refuse() {
    for case in 0..9 {
        let mut fixture = Fixture::new();
        match case {
            0 => fixture.envelope.schema = "other/1".into(),
            1 => fixture.envelope.source = "other".into(),
            2 => fixture.envelope.item = Handle::new(),
            3 => fixture.envelope.status = 600,
            4 => fixture.envelope.detected_media = Some("MEDIA_CANARY".into()),
            5 => {
                fixture.envelope.redirects = vec![
                    RedirectHop {
                        identity: fixture.envelope.requested.clone(),
                        status: 302
                    };
                    1001
                ];
            }
            6 => fixture.envelope.redirects.push(RedirectHop {
                identity: fixture.envelope.requested.clone(),
                status: 200,
            }),
            7 => {
                fixture.envelope.requested =
                    SafeIdentity::new("https://garden.example/docs/start?meaningful=other")
                        .unwrap();
            }
            _ => fixture.envelope.status = 99,
        }
        assert_eq!(
            fixture
                .db
                .prepare_capture(&fixture.context, &fixture.envelope, b"body"),
            Err(ReceiptError::Invalid),
            "case {case}"
        );
        assert!(fixture.db.artifact(&Digest::of(b"body")).unwrap().is_none());
    }
}
#[test]
fn n12_storage_allow_list_and_safe_value_guard_are_exact() {
    assert_eq!(
        safe_header_names().unwrap(),
        [
            "content-type",
            "content-length",
            "etag",
            "last-modified",
            "content-encoding"
        ]
    );
    for (name, value) in [
        ("authorization", "AUTH_CANARY"),
        ("etag", "VALIDATOR_CANARY"),
        ("content-type", "TYPE_CANARY"),
        ("content-length", "LENGTH_CANARY"),
        ("content-encoding", "CODEC_CANARY"),
    ] {
        let mut fixture = Fixture::new();
        fixture.envelope.headers.insert(
            name.into(),
            SafeHeader::Value {
                value: value.into(),
            },
        );
        let refused = fixture.prepare();
        assert!(
            refused.is_err(),
            "unsafe header value refuses before persistence"
        );
        let error = refused.unwrap_err();
        assert_eq!(error, ReceiptError::Invalid);
        assert!(!format!("{error:?} {error}").contains(value));
        assert!(fixture.db.artifact(&Digest::of(b"body")).unwrap().is_none());
    }
}
#[test]
fn n12_unknown_hashed_header_is_refused_not_masked_by_value_validation() {
    use maestro_kernel::acquisition::HeaderReason;
    let mut fixture = Fixture::new();
    fixture.envelope.headers.insert(
        "authorization".into(),
        SafeHeader::Hashed {
            digest: Digest::of(b"synthetic unsafe value"),
            reason: HeaderReason::UntrustedValue,
        },
    );
    assert_eq!(fixture.prepare(), Err(ReceiptError::Invalid));
    assert!(fixture.db.artifact(&Digest::of(b"body")).unwrap().is_none());
}
#[test]
fn n12_dom_wire_and_parent_labels_have_allowed_neighbours() {
    for (representation, transport, parent, allowed) in [
        (
            Representation::WireBody,
            Transport::BrowserRender,
            false,
            false,
        ),
        (
            Representation::WireBody,
            Transport::BrowserRequest,
            false,
            true,
        ),
        (Representation::WireBody, Transport::Http, true, false),
        (
            Representation::RenderedDom,
            Transport::BrowserRender,
            false,
            true,
        ),
        (
            Representation::RenderedDom,
            Transport::BrowserRender,
            true,
            false,
        ),
        (Representation::SelectedHtml, Transport::Http, true, false),
        (Representation::ApiRecord, Transport::Http, true, false),
    ] {
        let policy = super::n09_profiles::browser(match transport {
            Transport::Http => "http",
            Transport::BrowserRequest => "browser_request",
            Transport::BrowserRender => "browser_render",
        });
        let mut fixture = Fixture::with_policy(policy);
        fixture.envelope.representation = representation;
        fixture.envelope.transport = transport;
        fixture.envelope.parent = parent.then_some(fixture.envelope.inputs);
        assert_eq!(fixture.prepare().is_ok(), allowed);
    }
    let mut fixture = Fixture::new();
    fixture.envelope.transport = Transport::BrowserRequest;
    assert_eq!(
        http_envelope(&mut fixture.envelope, &response()),
        Err(ReceiptError::Invalid)
    );
}
#[test]
fn n12_different_bytes_cannot_replace_prepared_capture() {
    let mut fixture = Fixture::new();
    let original = fixture.prepare().unwrap();
    fixture.envelope.artifact = Digest::of(b"else");
    assert_eq!(
        fixture
            .db
            .prepare_capture(&fixture.context, &fixture.envelope, b"else"),
        Err(ReceiptError::Conflict)
    );
    assert!(fixture.db.read("reader", original).unwrap().is_some());
    assert!(fixture.db.artifact(&Digest::of(b"else")).unwrap().is_none());
}
#[test]
fn n12_missing_or_corrupt_artifact_never_acknowledges() {
    for corrupt in [false, true] {
        let fixture = Fixture::new();
        let handle = fixture.prepare().unwrap();
        let hex = fixture.envelope.artifact.as_str();
        let path = fixture
            .root
            .join("artifacts/sha256")
            .join(&hex[..2])
            .join(&hex[2..4])
            .join(hex);
        if corrupt {
            fs::write(path, b"corrupt").unwrap();
        } else {
            fs::remove_file(path).unwrap();
        }
        assert!(
            fixture
                .db
                .acknowledge_capture(&fixture.context, handle)
                .is_err()
        );
        assert!(fixture.prepare().is_err());
        assert!(
            Frontier::page(
                &fixture.db,
                &fixture.db.visible("reader").unwrap(),
                "notes",
                None,
                10
            )
            .unwrap()[0]
                .capture
                .is_none()
        );
    }
}
#[test]
fn n12_n11_budget_holds_before_write_and_replay_adds_no_usage() {
    let fixture = Fixture::new();
    let (_, resources) = n11_support::resources();
    let mut reservation = n11_support::reserve(&resources, Usage::default());
    let mut limits = n11_support::limits();
    limits.staging_bytes = 1.try_into().unwrap();
    let bounds = [limits];
    let mut budget = CaptureBudget {
        reservation: &mut reservation,
        bounds: &bounds,
        usage: Usage::default(),
    };
    assert!(
        prepare(
            &fixture.db,
            &fixture.policy,
            &fixture.context,
            (&fixture.envelope, b"body"),
            &mut budget
        )
        .is_err()
    );
    assert!(fixture.db.artifact(&Digest::of(b"body")).unwrap().is_none());
    let mut reservation = n11_support::reserve(&resources, Usage::default());
    let bounds = [n11_support::limits()];
    let mut budget = CaptureBudget {
        reservation: &mut reservation,
        bounds: &bounds,
        usage: Usage::default(),
    };
    let first = prepare(
        &fixture.db,
        &fixture.policy,
        &fixture.context,
        (&fixture.envelope, b"body"),
        &mut budget,
    )
    .unwrap();
    let usage = budget.usage;
    assert_eq!(
        usage.staging_bytes,
        4 + serde_json::to_vec(&fixture.envelope).unwrap().len() as u64
    );
    assert_eq!(
        prepare(
            &fixture.db,
            &fixture.policy,
            &fixture.context,
            (&fixture.envelope, b"body"),
            &mut budget
        )
        .unwrap(),
        first
    );
    assert_eq!(budget.usage, usage);
}

#[test]
fn n12_http_response_source_cannot_substitute_another_source() {
    let mut fixture = Fixture::new();
    fixture.envelope.source = "other".into();
    assert_eq!(
        http_envelope(&mut fixture.envelope, &response()),
        Err(ReceiptError::Invalid)
    );
}
#[test]
fn n12_wrong_item_capture_handle_refuses_before_ack() {
    let mut fixture = Fixture::new();
    let handle = fixture.prepare().unwrap();
    let other = fixture
        .db
        .enqueue(
            &fixture.context.writer,
            &NewItem {
                fetch_identity: "https://garden.example/docs/other".into(),
                authorization_context: fixture.envelope.authorization_context.clone(),
                representation_profile: fixture.envelope.profile.clone(),
            },
            fixture.context.now,
        )
        .unwrap();
    fixture.context.item = fixture
        .db
        .lease(
            &fixture.context.writer,
            other.id,
            DispatchRequest {
                lease: LeaseRequest {
                    holder: "worker",
                    now: fixture.context.now,
                    term: Duration::from_secs(30),
                },
                max_attempts: 3,
            },
        )
        .unwrap();
    assert_eq!(
        fixture.db.acknowledge_capture(&fixture.context, handle),
        Err(ReceiptError::Invalid)
    );
}
#[test]
fn n12_monotonic_and_durable_expiry_never_revive_a_capture() {
    for monotonic in [false, true] {
        let mut fixture = Fixture::new();
        let handle = fixture.prepare().unwrap();
        if monotonic {
            fixture.context.item.deadline = Instant::now();
        } else {
            fixture.context.now += Duration::from_secs(31);
        }
        assert_eq!(
            fixture.db.acknowledge_capture(&fixture.context, handle),
            Err(ReceiptError::Conflict)
        );
        assert_eq!(fixture.prepare(), Err(ReceiptError::Conflict));
    }
}
