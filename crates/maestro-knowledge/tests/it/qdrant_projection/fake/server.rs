//! The fake's server: its two services on a loopback port, served by the
//! runtime of the test that starts it.

use super::state::Fake;
use qdrant_client::qdrant::{collections_server::CollectionsServer, points_server::PointsServer};
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

    /// Makes it refuse the next call `call` with `code`: `upsert`, `count`,
    /// `get`, `create`, `collection_exists`, `collection_info` or
    /// `update_aliases`.
    pub(in super::super) fn refuse_next(&self, call: &'static str, code: Code) {
        self.fake.refuse_next(call, code);
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
}
