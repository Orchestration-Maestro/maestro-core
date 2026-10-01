//! A new generation indexes the scope payload it searches by.

use super::{backends::backends, kernel::Kernel, models};
use maestro_kernel::gateway::Role;
use maestro_knowledge::index::Projection;
use qdrant_client::qdrant::PayloadSchemaType;

#[tokio::test]
async fn newly_built_generations_have_a_keyword_scope_tags_index() {
    for backend in backends("newly_built_generations_have_a_keyword_scope_tags_index") {
        let kernel = Kernel::with_guides(1);
        let card = models::embedder(3);
        assert_eq!(card.fields().role, Role::Embedder);
        let port = models::Embedder::default();
        let qdrant = backend.client();
        let report = Projection {
            database: &kernel.database,
            scopes: &kernel.scopes,
            projection: &qdrant,
            port: &port,
            card: &card,
        }
        .publish(&kernel.chunk_set)
        .await
        .unwrap();

        let info = super::support::client(&backend)
            .collection_info(&report.qdrant_collection)
            .await
            .unwrap()
            .result
            .unwrap();
        let index = info.payload_schema.get("scope_tags").unwrap();
        assert_eq!(index.data_type, PayloadSchemaType::Keyword as i32);
        if backend.name == "qdrant" {
            super::support::client(&backend)
                .delete_collection(&report.qdrant_collection)
                .await
                .unwrap();
        }
    }
}
