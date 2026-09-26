//! Points: each chunk as its generation's collection holds it, under an ID
//! its chunk alone gives, with its two vectors and its payload.

use super::{
    error::Error,
    provenance::Provenance,
    qdrant::{DENSE, SPARSE},
};
use crate::lexical::SparseVector;
use maestro_kernel::chunk_set::Chunk;
use qdrant_client::{
    Payload,
    qdrant::{NamedVectors, PointStruct, Vector},
};
use serde_json::{Map, Value, json};
use sha1::{Digest as _, Sha1};
use uuid::{Builder, Uuid};

/// The namespace of point IDs: the `UUIDv5`, in the URL namespace, of
/// `https://github.com/Orchestration-Maestro/maestro-core#qdrant-point`.
const NAMESPACE: Uuid = Uuid::from_u128(0x8cfb_99af_c40a_5a27_a068_e1e2_97c9_12d2);

/// The ID of the point of the chunk `chunk_id`: the `UUIDv5` of the chunk's ID
/// in [`NAMESPACE`], hyphenated. A chunk's ID names one occurrence of a
/// passage, its scope, source revision, profiles and place (01 §7), so the
/// same chunk set gives the same point IDs, and writing a batch again
/// changes nothing.
pub(super) fn point_id(chunk_id: &str) -> String {
    let digest = Sha1::new()
        .chain_update(NAMESPACE.as_bytes())
        .chain_update(chunk_id)
        .finalize();
    let [bytes @ .., _, _, _, _] = digest.0;
    Builder::from_sha1_bytes(bytes)
        .into_uuid()
        .hyphenated()
        .to_string()
}

/// The point of `chunk`: its dense vector `dense`, its sparse vector
/// `sparse`, empty when there is none, and the payload of plan D9, from what
/// `provenance` says of its revision: its ID and its revision's, the heading
/// path of its section, empty when it has none, its scope tags, and its
/// revision's version and source kind, null when the revision has none as
/// text.
///
/// # Errors
///
/// [`Error::Unreadable`] when its section is not one of its revision's.
pub(super) fn point(
    chunk: &Chunk,
    provenance: &Provenance,
    dense: &[f32],
    sparse: Option<&SparseVector>,
) -> Result<PointStruct, Error> {
    let (indices, values) = sparse.map_or((&[][..], &[][..]), |vector| {
        (vector.indices(), vector.values())
    });
    let vectors = NamedVectors::default()
        .add_vector(DENSE, Vector::new_dense(dense))
        .add_vector(SPARSE, Vector::new_sparse(indices, values));
    let payload: Map<String, Value> = [
        ("chunk_id", json!(chunk.id)),
        ("revision_id", json!(chunk.revision_id)),
        ("section_path", json!(provenance.section_path(chunk)?)),
        ("scope_tags", json!(provenance.scope_tags)),
        ("version", json!(provenance.version)),
        ("source_kind", json!(provenance.source_kind)),
    ]
    .into_iter()
    .map(|(field, value)| (field.to_owned(), value))
    .collect();
    Ok(PointStruct::new(
        point_id(&chunk.id),
        vectors,
        Payload::from(payload),
    ))
}
