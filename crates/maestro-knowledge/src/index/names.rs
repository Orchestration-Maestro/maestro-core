//! Qdrant names derived from a generation's kernel record.

use maestro_kernel::generation::Generation;

/// The Qdrant collection name of `generation`.
pub(crate) fn collection_name(generation: &Generation) -> String {
    format!("maestro-{}-g{}", generation.collection_id, generation.id)
}

/// The Qdrant alias name of `generation`'s collection.
pub(crate) fn alias_name(generation: &Generation) -> String {
    format!("maestro-{}", generation.collection_id)
}

#[cfg(test)]
mod tests {
    use super::{alias_name, collection_name};
    use maestro_kernel::generation::{Generation, GenerationState};

    #[test]
    fn a_generation_uses_the_expected_qdrant_collection_and_alias_names() {
        let generation = Generation {
            id: 7,
            collection_id: "ctm".to_owned(),
            chunk_set_id: "chunks".to_owned(),
            embedding_profile: "dense-profile".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
            state: GenerationState::Building,
            point_count: None,
            published_at: None,
        };
        assert_eq!(collection_name(&generation), "maestro-ctm-g7");
        assert_eq!(alias_name(&generation), "maestro-ctm");
    }
}
