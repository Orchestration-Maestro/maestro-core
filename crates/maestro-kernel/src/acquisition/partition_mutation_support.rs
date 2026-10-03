//! Synthetic partition fixtures shared by mutation regression tests.
use super::{
    Batch, CaptureContext, CaptureEnvelope, ChangeKeys, DiscoveredItem, Enumeration, Frontier,
    Handle, LeaseRequest, NewItem, Partition, Receipts, Representation, SafeIdentity, SourceLease,
    Transport, Window,
};
use crate::{artifact::Digest, scope::Scope, store::Database};
use maestro_test_scratch::scratch_directory;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Database closes before its test-owned directory is removed.
pub(super) struct Fixture {
    pub(super) db: Database,
    pub(super) scope: Scope,
    pub(super) writer: SourceLease,
    _root: Scratch,
}
impl Fixture {
    /// Open a leased synthetic source.
    pub(super) fn new() -> Self {
        let root = scratch_directory().unwrap();
        let db = Database::open_in(&root).unwrap();
        let scope = "workspace/default/collection/partition".parse().unwrap();
        let writer = db.lease_source("docs", &scope, lease()).unwrap();
        Self {
            db,
            scope,
            writer,
            _root: Scratch(root),
        }
    }
    /// Frozen capture for the supplied request, with a live dispatch.
    pub(super) fn capture(&self, request: &NewItem) -> (CaptureContext, CaptureEnvelope) {
        let row = self.db.enqueue(&self.writer, request, now()).unwrap();
        let item = self
            .db
            .lease(
                &self.writer,
                row.id,
                super::DispatchRequest {
                    lease: lease(),
                    max_attempts: 3,
                },
            )
            .unwrap();
        let inputs = self.db.retain(&self.scope, b"inputs", &[]).unwrap();
        let identity = SafeIdentity::new(&request.fetch_identity).unwrap();
        let envelope = CaptureEnvelope {
            schema: "maestro-capture/1".into(),
            source: "docs".into(),
            item: row.id.into(),
            run: Handle::new(),
            requested: identity.clone(),
            final_identity: identity,
            redirects: vec![],
            status: 200,
            headers: BTreeMap::new(),
            declared_media: None,
            detected_media: None,
            artifact: Digest::of(b"body"),
            length: 4,
            observed_ms: 1_000_000,
            transport: Transport::Http,
            profile: request.representation_profile.clone(),
            authorization_context: request.authorization_context.clone(),
            representation: Representation::WireBody,
            parent: None,
            inputs,
            access: inputs,
            decision: inputs,
        };
        (
            CaptureContext {
                writer: self.writer.clone(),
                item,
                now: now(),
            },
            envelope,
        )
    }
}
/// Removed after the database fields have closed their connections.
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
/// Fixed lease time; no real sleeps.
pub(super) fn now() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1000)
}
/// Finite writer/dispatch term.
pub(super) fn lease() -> LeaseRequest<'static> {
    LeaseRequest {
        holder: "worker",
        now: now(),
        term: Duration::from_secs(3600),
    }
}
/// Request keys with unknown source change evidence.
pub(super) fn keys() -> ChangeKeys {
    ChangeKeys {
        revision: None,
        validator: None,
        metadata: None,
        permissions: Digest::of(b"account"),
        links: None,
        representation: None,
    }
}
/// Unique HTTPS identity in one effective context.
pub(super) fn discovered(index: usize) -> DiscoveredItem {
    DiscoveredItem {
        request: NewItem {
            fetch_identity: format!("https://example.test/item/{index}"),
            authorization_context: keys().permissions,
            representation_profile: Digest::of(b"wire"),
        },
        keys: keys(),
    }
}
/// Complete empty index; tests change one independent claim at a time.
pub(super) fn batch() -> Batch {
    Batch {
        partition: Partition {
            id: Handle::new(),
            run: Handle::new(),
            kind: Enumeration::Index,
            window: Window {
                start: 10,
                end: 20,
                overlap: 0,
                skew: 0,
            },
            max_batches: 2,
            max_items: 2,
        },
        cursor: None,
        next: None,
        terminal: true,
        verification_final: false,
        stable: true,
        truncated: false,
        expected: Some(0),
        items: vec![],
        extractor: None,
        parent_keys: None,
        not_enqueued: vec![],
        inventory_overflow: 0,
        parent_depth: None,
        capture: None,
    }
}
