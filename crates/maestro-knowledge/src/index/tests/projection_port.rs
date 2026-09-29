//! Contract checks for a backend-neutral retrieval projection port.

use crate::index::projection_port::{
    CollectionLayout, PointHit, ProjectionFilter, ProjectionPage, ProjectionPoint,
    RetrievalProjectionPort, SparseValues,
};
use std::collections::BTreeMap;

#[test]
fn fake_port_implements_the_backend_neutral_contract() {
    fn accepts_port<T: RetrievalProjectionPort<Error = String>>() {}
    accepts_port::<FakePort>();
}

#[derive(Debug)]
struct FakePort;

impl RetrievalProjectionPort for FakePort {
    type Error = String;

    async fn collection_exists(&self, _: &str) -> Result<bool, Self::Error> {
        Err("unused".into())
    }
    async fn create_collection(&self, _: &str, _: CollectionLayout) -> Result<(), Self::Error> {
        Err("unused".into())
    }
    async fn collection_layout(&self, _: &str) -> Result<Option<CollectionLayout>, Self::Error> {
        Err("unused".into())
    }
    async fn index_payload_fields(&self, _: &str) -> Result<(), Self::Error> {
        Err("unused".into())
    }
    async fn payload_fields(&self, _: &str) -> Result<BTreeMap<String, String>, Self::Error> {
        Err("unused".into())
    }
    async fn upsert(&self, _: &str, _: Vec<ProjectionPoint>) -> Result<(), Self::Error> {
        Err("unused".into())
    }
    async fn count(&self, _: &str) -> Result<u64, Self::Error> {
        Err("unused".into())
    }
    async fn found(&self, _: &str, _: &[String]) -> Result<Vec<String>, Self::Error> {
        Err("unused".into())
    }
    async fn payloads(&self, _: &str, _: &[String]) -> Result<Vec<PointHit>, Self::Error> {
        Err("unused".into())
    }
    async fn query_dense(
        &self,
        _: &str,
        _: Vec<f32>,
        _: usize,
        _: ProjectionFilter,
    ) -> Result<Vec<PointHit>, Self::Error> {
        Err("unused".into())
    }
    async fn query_sparse(
        &self,
        _: &str,
        _: SparseValues,
        _: usize,
        _: ProjectionFilter,
    ) -> Result<Vec<PointHit>, Self::Error> {
        Err("unused".into())
    }
    async fn scroll(
        &self,
        _: &str,
        _: ProjectionFilter,
        _: Option<String>,
    ) -> Result<ProjectionPage, Self::Error> {
        Err("unused".into())
    }
    async fn alias_target(&self, _: &str) -> Result<Option<String>, Self::Error> {
        Err("unused".into())
    }
    async fn replace_alias(&self, _: &str, _: &str) -> Result<(), Self::Error> {
        Err("unused".into())
    }
    async fn delete_collection(&self, _: &str) -> Result<(), Self::Error> {
        Err("unused".into())
    }
}
