//! Tests for the published vector-layout diagnostic.

use crate::index::{CollectionLayout, DenseDistance, DenseLayout, SparseModifier, Unverified};

#[test]
fn vector_mismatch_preserves_backend_layout_names() {
    let layout = CollectionLayout {
        dense: Some(DenseLayout {
            dimensions: 8,
            distance: DenseDistance::Euclid,
        }),
        sparse_present: true,
        sparse_modifier: Some(SparseModifier::None),
    };
    assert_eq!(
        super::super::verify::vectors(&layout, 8),
        Err(Unverified::Vectors {
            dimensions: 8,
            found: concat!(
                "a dense vector `dense` of 8 dimensions compared by Euclid and ",
                "a sparse vector `bm25` weighted by None modifier"
            )
            .to_owned(),
        })
    );

    let layout = CollectionLayout {
        dense: Some(DenseLayout {
            dimensions: 8,
            distance: DenseDistance::Cosine,
        }),
        sparse_present: false,
        sparse_modifier: None,
    };
    assert_eq!(
        super::super::verify::vectors(&layout, 8),
        Err(Unverified::Vectors {
            dimensions: 8,
            found: concat!(
                "a dense vector `dense` of 8 dimensions compared by Cosine and ",
                "no sparse vector `bm25`"
            )
            .to_owned(),
        })
    );
}
