//! Descriptor adapter over the existing retrieval projection infrastructure.

use super::{
    build::refused,
    port::{DescriptorProjection, DescriptorQuery, EmbeddedDescriptors},
    types::{BUILDER_VERSION, Descriptor, DescriptorError, DescriptorReceipt},
};
use crate::index::{
    CollectionLayout, PointHit, ProjectionFilter, ProjectionPoint, Qdrant, RetrievalProjectionPort,
    SparseValues, point_id,
};
use maestro_kernel::artifact::Digest;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The descriptor adapter reuses the existing configured Qdrant client.
#[derive(Debug)]
pub struct DescriptorQdrant<'a> {
    /// Caller-supplied existing vector infrastructure.
    projection: &'a Qdrant,
}

impl<'a> DescriptorQdrant<'a> {
    /// Bind the optional projection to the existing configured client.
    #[must_use]
    pub const fn new(projection: &'a Qdrant) -> Self {
        Self { projection }
    }
}

impl DescriptorProjection for DescriptorQdrant<'_> {
    async fn rebuild(&self, output: &EmbeddedDescriptors) -> Result<(), DescriptorError> {
        rebuild(self.projection, output).await?;
        self.projection
            .index_keyword_fields(
                &collection_name(&output.receipt),
                &[
                    "receipt_digest",
                    "collection_id",
                    "generation",
                    "version",
                    "eligibility",
                    "kind",
                ],
            )
            .await
            .map_err(|_| refused("descriptor payload indexing failed"))
    }
    async fn verify(&self, output: &EmbeddedDescriptors) -> Result<(), DescriptorError> {
        verify(self.projection, output).await
    }
    async fn delete(&self, receipt: &DescriptorReceipt) -> Result<(), DescriptorError> {
        self.projection
            .delete_owned_collection(&collection_name(receipt))
            .await
            .map_err(|_| refused("descriptor collection deletion failed"))
    }
    async fn lookup(
        &self,
        receipt: &DescriptorReceipt,
        query: DescriptorQuery,
    ) -> Result<Vec<PointHit>, DescriptorError> {
        lookup(self.projection, receipt, query).await
    }
}

/// Owned physical name binds the complete scope/content/model/profile receipt.
fn collection_name(receipt: &DescriptorReceipt) -> String {
    format!("maestro-descriptors-{}", receipt_digest(receipt).as_str())
}

/// Canonical serialized receipt identity for payload filtering and collection ownership.
fn receipt_digest(receipt: &DescriptorReceipt) -> Digest {
    Digest::of(json!(receipt).to_string().as_bytes())
}

/// Reuse the existing dense/sparse transport layout, with no sparse descriptor route.
fn layout(receipt: &DescriptorReceipt) -> CollectionLayout {
    CollectionLayout {
        dense_dimensions: u64::try_from(receipt.profile.dimensions).unwrap_or(u64::MAX),
        dense_present: true,
        dense_distance: "Cosine".into(),
        sparse_present: true,
        sparse_modifier: Some("Idf".into()),
    }
}

/// Canonical payloads carry all pointers, qualifiers and the disposable receipt.
fn points(output: &EmbeddedDescriptors) -> Vec<ProjectionPoint> {
    output
        .documents
        .iter()
        .zip(&output.vectors)
        .map(|(document, vector)| {
            let mut payload: BTreeMap<String, Value> =
                serde_json::from_value(json!(document)).unwrap_or_default();
            payload.extend([
                ("receipt".into(), json!(output.receipt)),
                (
                    "receipt_digest".into(),
                    json!(receipt_digest(&output.receipt)),
                ),
                ("collection_id".into(), json!(document.pin.collection_id)),
                (
                    "generation".into(),
                    json!(document.pin.generation_id.to_string()),
                ),
                (
                    "version".into(),
                    json!(json!(document.pin.version).to_string()),
                ),
                ("eligibility".into(), json!(document.eligible.to_string())),
            ]);
            ProjectionPoint {
                id: point_id(&format!("descriptor:{}", document.id.as_str())),
                dense: vector.clone(),
                sparse: SparseValues::default(),
                payload,
            }
        })
        .collect()
}

/// Populate a digest-bound projection and verify it before returning readiness.
pub(super) async fn rebuild<P: RetrievalProjectionPort>(
    projection: &P,
    output: &EmbeddedDescriptors,
) -> Result<(), DescriptorError> {
    let name = collection_name(&output.receipt);
    if !projection
        .collection_exists(&name)
        .await
        .map_err(|_| refused("descriptor projection read failed"))?
    {
        projection
            .create_collection(&name, layout(&output.receipt))
            .await
            .map_err(|_| refused("descriptor collection creation failed"))?;
    }
    projection
        .upsert_points(&name, points(output))
        .await
        .map_err(|_| refused("descriptor upsert failed"))?;
    verify(projection, output).await
}

/// Verify complete canonical payload equality rather than ANN storage bytes.
pub(super) async fn verify<P: RetrievalProjectionPort>(
    projection: &P,
    output: &EmbeddedDescriptors,
) -> Result<(), DescriptorError> {
    let receipt = &output.receipt;
    if receipt.profile.builder != Digest::of(BUILDER_VERSION.as_bytes())
        || receipt.content != Digest::of(json!(output.documents).to_string().as_bytes())
    {
        return Err(refused("invalid descriptor receipt"));
    }
    let name = collection_name(receipt);
    let found_layout = projection
        .collection_layout(&name)
        .await
        .map_err(|_| refused("descriptor layout read failed"))?;
    if found_layout != Some(layout(receipt)) {
        return Err(refused("incompatible descriptor layout"));
    }
    let count = projection
        .count_points(&name)
        .await
        .map_err(|_| refused("descriptor count failed"))?;
    if count != u64::try_from(receipt.count).unwrap_or(u64::MAX) {
        return Err(refused("descriptor count mismatch"));
    }
    let expected: BTreeMap<_, _> = points(output)
        .into_iter()
        .map(|point| (point.id, point.payload))
        .collect();
    let ids: Vec<_> = expected.keys().cloned().collect();
    let found = projection
        .payloads(&name, &ids)
        .await
        .map_err(|_| refused("descriptor payload read failed"))?;
    let found: BTreeMap<_, _> = found
        .into_iter()
        .map(|point| (point.id, point.payload))
        .collect();
    if found != expected {
        return Err(refused("descriptor canonical payload mismatch"));
    }
    Ok(())
}

/// Scoped backend query, never a post-filtered top-k.
pub(super) async fn lookup<P: RetrievalProjectionPort>(
    projection: &P,
    receipt: &DescriptorReceipt,
    query: DescriptorQuery,
) -> Result<Vec<PointHit>, DescriptorError> {
    if query.limit == 0 || !matches!(query.kind.as_str(), "claim" | "entity") {
        return Err(refused("descriptor scope cannot be represented"));
    }
    let filter = ProjectionFilter::All(
        [
            (
                "receipt_digest",
                receipt_digest(receipt).as_str().to_owned(),
            ),
            ("collection_id", receipt.pin.collection_id.clone()),
            ("generation", receipt.pin.generation_id.to_string()),
            ("version", json!(receipt.pin.version).to_string()),
            ("eligibility", "true".to_owned()),
            ("kind", query.kind),
        ]
        .into_iter()
        .map(|(field, value)| ProjectionFilter::ExactString {
            field: field.into(),
            value,
        })
        .collect(),
    );
    let hits = projection
        .search_dense(&collection_name(receipt), query.vector, query.limit, filter)
        .await
        .map_err(|_| refused("descriptor lookup unavailable"))?;
    for hit in &hits {
        let document: Descriptor = serde_json::from_value(json!(hit.payload))
            .map_err(|_| refused("invalid descriptor canonical payload"))?;
        // The keyword is only a transport encoding, never an eligibility authority.
        if hit.payload.get("eligibility") != Some(&json!(document.eligible.to_string())) {
            return Err(refused("descriptor eligibility encoding mismatch"));
        }
    }
    Ok(hits)
}
