//! Kernel-owned capture fixtures use only the public durable ports.
#![cfg(test)]
use super::n04_frontier_support::{Scratch, dispatch, item, now, request};
use maestro_kernel::{
    acquisition::{
        CaptureContext, CaptureEnvelope, Captures, Frontier, Handle, Item, Receipts,
        Representation, SafeIdentity, Transport,
    },
    artifact::Digest,
    scope::{Right, Scope},
    store::Database,
};
use rusqlite::Connection;
use std::{collections::BTreeMap, path::PathBuf};

/// Database closes before the owned scratch directory is removed.
pub(super) struct Fixture {
    pub(super) db: Database,
    pub(super) root: Scratch,
    pub(super) scope: Scope,
    pub(super) context: CaptureContext,
    pub(super) envelope: CaptureEnvelope,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let root = Scratch::new();
        let db = Database::open_in(&root).unwrap();
        let scope: Scope = "workspace/default/collection/docs/source/docs"
            .parse()
            .unwrap();
        db.grant("reader", &scope, Right::Read, "owner").unwrap();
        let writer = db.lease_source("docs", &scope, request(now())).unwrap();
        let row = db.enqueue(&writer, &item(), now()).unwrap();
        let lease = db.lease(&writer, row.id, dispatch(now())).unwrap();
        let inputs = db.retain(&scope, b"frozen inputs", &[]).unwrap();
        let identity = SafeIdentity::new(&row.request.fetch_identity).unwrap();
        let envelope = CaptureEnvelope {
            schema: "maestro-capture/1".into(),
            source: "docs".into(),
            item: row.id.to_string().parse().unwrap(),
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
            observed_ms: 1_000_000_000,
            transport: Transport::Http,
            profile: row.request.representation_profile,
            authorization_context: row.request.authorization_context,
            representation: Representation::WireBody,
            parent: None,
            inputs,
            access: inputs,
            decision: inputs,
        };
        Self {
            db,
            root,
            scope,
            context: CaptureContext {
                writer,
                item: lease,
                now: now(),
            },
            envelope,
        }
    }
    pub(super) fn prepare(&self) -> Handle {
        self.db
            .prepare_capture(&self.context, &self.envelope, b"body", u64::MAX)
            .unwrap()
            .handle
    }
    pub(super) fn acknowledge(&self, handle: Handle) {
        self.db.acknowledge_capture(&self.context, handle).unwrap();
    }
    pub(super) fn row(&self) -> Item {
        Frontier::page(
            &self.db,
            &self.db.visible("reader").unwrap(),
            "docs",
            None,
            100,
        )
        .unwrap()
        .into_iter()
        .find(|row| row.id == self.context.item.item)
        .unwrap()
    }
    pub(super) fn sql(&self) -> Connection {
        Connection::open(self.root.join("kernel.sqlite3")).unwrap()
    }
    pub(super) fn artifact_path(&self, digest: &Digest) -> PathBuf {
        let hex = digest.as_str();
        self.root
            .join("artifacts/sha256")
            .join(hex.get(..2).unwrap())
            .join(hex.get(2..4).unwrap())
            .join(hex)
    }
    pub(super) fn derive(&mut self, parent: Handle, kind: Representation) {
        let mut request = item();
        request.fetch_identity = format!("https://example.test/child/{parent}");
        let row = self
            .db
            .enqueue(&self.context.writer, &request, now())
            .unwrap();
        self.context.item = self
            .db
            .lease(&self.context.writer, row.id, dispatch(now()))
            .unwrap();
        self.envelope.item = row.id.to_string().parse().unwrap();
        self.envelope.requested = SafeIdentity::new(&request.fetch_identity).unwrap();
        self.envelope.final_identity = self.envelope.requested.clone();
        self.envelope.parent = Some(parent);
        self.envelope.representation = kind;
    }
}
