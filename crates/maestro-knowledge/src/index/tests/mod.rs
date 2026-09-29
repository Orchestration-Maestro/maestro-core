//! Tests of the projection's parts that a publication through the fake or a
//! real Qdrant cannot reach (`tests/it/qdrant_projection`): each refusal's
//! words and cause, an embedder that never answers, the lengths of no
//! passage, point IDs against Python's `uuid`, and a Qdrant out of reach.

mod dense;
mod errors;
mod lengths;
mod point;
mod qdrant;
