//! The fake's server: its two services on a loopback port, served by the
//! runtime of the test that starts it.

use super::state::Fake;
use qdrant_client::qdrant::{
    Filter, PointId, Value, collections_server::CollectionsServer, points_server::PointsServer,
    value::Kind,
};
use std::sync::Arc;
use tokio::sync::Notify;
use tonic::{
    Code,
    transport::{Server, server::TcpIncoming},
};

/// A running fake.
#[derive(Debug, Clone)]
pub(in super::super) struct FakeQdrant {
    /// Where its gRPC API answers.
    url: String,
    /// What it keeps.
    fake: Fake,
}

impl FakeQdrant {
    /// A fake with nothing in it, on a new loopback port, served by the
    /// runtime of the calling test.
    pub(in super::super) fn serve() -> Self {
        let incoming = TcpIncoming::bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let url = format!("http://{}", incoming.local_addr().unwrap());
        let fake = Fake::default();
        let server = Server::builder()
            .add_service(CollectionsServer::new(fake.clone()))
            .add_service(PointsServer::new(fake.clone()))
            .serve_with_incoming(incoming);
        drop(tokio::spawn(server));
        Self { url, fake }
    }

    /// Where its gRPC API answers.
    pub(in super::super) fn url(&self) -> &str {
        &self.url
    }

    /// Points `alias` at the existing collection `collection`, as an
    /// operator's mistaken alias change would.
    pub(in super::super) fn alias_to(&self, alias: &str, collection: &str) {
        assert!(self.fake.state().collections.contains_key(collection));
        self.fake
            .state()
            .aliases
            .insert(alias.to_owned(), collection.to_owned());
    }

    /// Blocks the next payload scroll until a test releases its gate.
    pub(in super::super) fn gate_next_scroll(&self) -> Arc<Notify> {
        self.fake.gate_next_scroll()
    }

    /// Makes it refuse the next call `call` with `code`: `upsert`, `count`,
    /// `get`, `create`, `collection_exists`, `collection_info` or
    /// `update_aliases`.
    pub(in super::super) fn refuse_next(&self, call: &'static str, code: Code) {
        self.fake.refuse_next(call, code);
    }

    /// Makes the next `count` calls of one kind refuse with `code`.
    pub(in super::super) fn refuse_next_n(&self, call: &'static str, code: Code, count: usize) {
        for _ in 0..count {
            self.fake.refuse_next(call, code);
        }
    }

    /// Makes the next `CreateAlias` action refuse.
    pub(in super::super) fn refuse_create_alias_next(&self) {
        self.fake.refuse_create_alias_next();
    }

    /// Makes it answer the next call `call`, `count` or `collection_info`,
    /// without its result.
    pub(in super::super) fn hollow_next(&self, call: &'static str) {
        self.fake.hollow_next(call);
    }

    /// Returns the exact filters sent to payload scrolls, in call order.
    pub(in super::super) fn scroll_filters(&self) -> Vec<Filter> {
        self.fake.state().scroll_filters.clone()
    }

    /// Returns every fake-service request, in call order.
    pub(in super::super) fn calls(&self) -> Vec<String> {
        self.fake.state().calls.clone()
    }

    /// Replaces one string payload field on a stored point.
    pub(in super::super) fn set_payload_text(
        &self,
        collection: &str,
        point_id: &str,
        field: &str,
        value: &str,
    ) {
        let mut state = self.fake.state();
        let point = state
            .collection(collection)
            .unwrap()
            .points
            .get_mut(&super::state::key(&PointId::from(point_id)))
            .unwrap();
        point.payload.insert(
            field.to_owned(),
            Value {
                kind: Some(Kind::StringValue(value.to_owned())),
            },
        );
    }

    /// The target of `alias` in the fake's current state.
    pub(in super::super) fn alias_target(&self, alias: &str) -> Option<String> {
        self.fake.state().aliases.get(alias).cloned()
    }
}
