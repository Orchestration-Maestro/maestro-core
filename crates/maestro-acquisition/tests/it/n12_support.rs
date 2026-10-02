//! Synthetic scoped capture fixtures; no live authority is granted.
#![expect(
    clippy::indexing_slicing,
    reason = "authored synthetic inventory positions"
)]
use super::{n09_support, n11_support};
use maestro_acquisition::CheckedPolicy;
use maestro_acquisition::capture::{
    CaptureBudget, CaptureContext, CaptureEnvelope, Representation, SafeIdentity, Transport,
    prepare,
};
use maestro_acquisition::transport::{budget::Usage, http::Response, stream::Accounting};
use maestro_kernel::{
    acquisition::{
        DispatchRequest, Frontier, Handle, LeaseRequest, NewItem, ReceiptError, Receipts,
    },
    artifact::Digest,
    scope::{Right, Scope},
    store::Database,
};
use maestro_test_scratch::scratch_directory;
use std::{
    collections::BTreeMap,
    fs,
    ops::Deref,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Test-owned durable kernel directory.
pub(super) struct Fixture {
    pub(super) db: Database,
    pub(super) root: Scratch,
    pub(super) context: CaptureContext,
    pub(super) policy: CheckedPolicy,
    pub(super) envelope: CaptureEnvelope,
}
impl Fixture {
    pub(super) fn new() -> Self {
        Self::with_policy(n09_support::policy())
    }
    pub(super) fn with_policy(policy: CheckedPolicy) -> Self {
        let root = Scratch::new();
        let db = Database::open_in(&root).unwrap();
        let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
        db.grant("reader", &scope, Right::Read, "owner").unwrap();
        let now = now();
        let lease = LeaseRequest {
            holder: "worker",
            now,
            term: Duration::from_secs(30),
        };
        let writer = db.lease_source("notes", &scope, lease).unwrap();
        let profile = policy.policy().sources[0]
            .acquisition_profile
            .digest
            .clone();
        let authorization_context = Digest::of(b"public");
        let item = db
            .enqueue(
                &writer,
                &NewItem {
                    fetch_identity: "https://garden.example/docs/start".into(),
                    authorization_context: authorization_context.clone(),
                    representation_profile: profile.clone(),
                },
                now,
            )
            .unwrap();
        let dispatch = db
            .lease(
                &writer,
                item.id,
                DispatchRequest {
                    lease,
                    max_attempts: 3,
                },
            )
            .unwrap();
        let inputs = db.retain(&scope, b"frozen inputs", &[]).unwrap();
        let envelope = CaptureEnvelope {
            schema: "maestro-capture/1".into(),
            source: "notes".into(),
            item: item.id.to_string().parse().unwrap(),
            run: Handle::new(),
            requested: SafeIdentity::new("https://garden.example/docs/start").unwrap(),
            final_identity: SafeIdentity::new("https://garden.example/docs/start").unwrap(),
            redirects: vec![],
            status: 200,
            headers: BTreeMap::new(),
            declared_media: None,
            detected_media: None,
            artifact: Digest::of(b"body"),
            length: 4,
            observed_ms: 1_000_000_000,
            transport: Transport::Http,
            profile,
            authorization_context,
            representation: Representation::WireBody,
            parent: None,
            inputs,
            access: inputs,
            decision: inputs,
        };
        Self {
            root,
            db,
            policy,
            context: CaptureContext {
                writer,
                item: dispatch,
                now,
            },
            envelope,
        }
    }
    pub(super) fn prepare(&self) -> Result<Handle, ReceiptError> {
        let (_, resources) = n11_support::resources();
        let mut reservation = n11_support::reserve(&resources, Usage::default());
        let bounds = [n11_support::limits()];
        let mut budget = CaptureBudget {
            carried_staging: None,
            reservation: &mut reservation,
            bounds: &bounds,
            usage: Usage::default(),
        };
        prepare(
            &self.db,
            &self.policy,
            &self.context,
            (&self.envelope, b"body"),
            &mut budget,
        )
    }
}
/// Directory owner drops after the database field closes all connections.
pub(super) struct Scratch(PathBuf);
impl Scratch {
    pub(super) fn new() -> Self {
        Self(scratch_directory().unwrap())
    }
}
impl Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
/// Fixed authority clock.
pub(super) fn now() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_000_000)
}
/// Actual N09 response, not a parallel response fixture.
pub(super) fn response() -> Response {
    let mut result = None;
    n09_support::run(async {
        let policy = n09_support::policy();
        let controls = super::n07_parse_url_identity_and_denial_precedence::Controls::default();
        let grants = n09_support::Grants::default();
        let dns = n09_support::Dns::default();
        let wire = n09_support::Wire::new(vec![n09_support::response(
            200,
            "Content-Type: text/html; token=HEADER_CANARY\r\n\
             ETag: VALIDATOR_CANARY\r\nLast-Modified: DATE_CANARY\r\n\
             Set-Cookie: COOKIE_CANARY\r\nAuthorization: AUTH_CANARY\r\n\
             X-Trace: TRACE_CANARY\r\n",
            b"body",
        )]);
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        result = Some(
            n09_support::http(&policy, &controls, &grants, &dns, &wire)
                .fetch(
                    &n09_support::fetch("https://garden.example/docs/start"),
                    &mut accounting,
                )
                .await
                .unwrap(),
        );
    });
    result.unwrap()
}
