//! Checked profiles cannot be bypassed by changing both provenance labels.
use super::{n09_profiles::browser, n12_support::Fixture};
use maestro_acquisition::{
    capture::{Representation, Transport},
    policy::{acquisition::AcquisitionProfile, schema::SourcePolicy},
};
use maestro_kernel::{acquisition::ReceiptError, artifact::Digest};

#[test]
fn n12_http_profile_cannot_be_relabelled_as_rendered_dom() {
    let mut fixture = Fixture::new();
    fixture.envelope.transport = Transport::BrowserRender;
    fixture.envelope.representation = Representation::RenderedDom;
    assert_eq!(fixture.prepare(), Err(ReceiptError::Invalid));
    let mut rendered = Fixture::with_policy(browser("browser_render"));
    rendered.envelope.transport = Transport::BrowserRender;
    rendered.envelope.representation = Representation::RenderedDom;
    assert!(rendered.prepare().is_ok());
}
#[test]
fn n12_transport_definition_move_changes_no_schema_or_wire_bytes() {
    assert_eq!(
        serde_json::to_vec(&schemars::schema_for!(SourcePolicy)).unwrap(),
        include_bytes!("../fixtures/n12-policy-schema.json")
            .strip_suffix(b"\n")
            .expect("golden fixture ends with exactly one LF")
    );
    assert_eq!(
        serde_json::to_vec(&schemars::schema_for!(AcquisitionProfile)).unwrap(),
        include_bytes!("../fixtures/n12-profile-schema.json")
            .strip_suffix(b"\n")
            .expect("golden fixture ends with exactly one LF")
    );
    assert_eq!(
        serde_json::to_vec(&[
            Transport::Http,
            Transport::BrowserRequest,
            Transport::BrowserRender
        ])
        .unwrap(),
        br#"["http","browser_request","browser_render"]"#
    );
    let profile: AcquisitionProfile =
        serde_json::from_slice(include_bytes!("../fixtures/http.json")).unwrap();
    let bytes = serde_json::to_vec(&profile).unwrap();
    assert_eq!(
        serde_json::from_slice::<AcquisitionProfile>(&bytes).unwrap(),
        profile
    );
}

#[test]
fn n12_current_policy_profile_digest_not_only_transport_is_bound() {
    let mut fixture = Fixture::new();
    let (mut collection, mut catalog) =
        super::n07_parse_url_identity_and_denial_precedence::fixture();
    super::support::put(
        &mut catalog,
        "policy",
        &serde_json::to_value(fixture.policy.policy()).unwrap(),
    );
    let mut profile = super::support::value(&catalog, "http");
    profile["version"] = 2.into();
    super::support::put(&mut catalog, "http", &profile);
    super::support::rebind(&mut collection, &mut catalog);
    fixture.policy =
        super::n07_parse_url_identity_and_denial_precedence::checked(collection, &catalog).unwrap();
    assert_eq!(fixture.prepare(), Err(ReceiptError::Invalid));
}
#[test]
fn n12_kernel_checks_profile_even_when_a_trusted_host_bypasses_projection() {
    use maestro_acquisition::capture::Captures as _;
    let mut fixture = Fixture::new();
    fixture.envelope.profile = Digest::of(b"other profile");
    assert_eq!(
        fixture
            .db
            .prepare_capture(&fixture.context, &fixture.envelope, b"body"),
        Err(ReceiptError::Invalid)
    );
}
#[test]
fn n12_unlisted_source_never_borrows_another_sources_checked_profile() {
    use maestro_kernel::acquisition::{DispatchRequest, Frontier as _, LeaseRequest, NewItem};
    use std::time::Duration;
    let mut fixture = Fixture::new();
    let lease = LeaseRequest {
        holder: "worker",
        now: fixture.context.now,
        term: Duration::from_secs(30),
    };
    fixture.context.writer = fixture
        .db
        .lease_source(
            "other",
            &"workspace/default/collection/garden".parse().unwrap(),
            lease,
        )
        .unwrap();
    let item = fixture
        .db
        .enqueue(
            &fixture.context.writer,
            &NewItem {
                fetch_identity: "https://garden.example/docs/start".into(),
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
            item.id,
            DispatchRequest {
                lease,
                max_attempts: 3,
            },
        )
        .unwrap();
    fixture.envelope.source = "other".into();
    fixture.envelope.item = item.id.to_string().parse().unwrap();
    assert_eq!(fixture.prepare(), Err(ReceiptError::Invalid));
}
