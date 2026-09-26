//! A fake Qdrant: a gRPC server on a loopback port, run by the test's
//! runtime, that keeps collections, points and aliases in memory and answers
//! the calls a publication and these tests make as Qdrant 1.19 does, as far
//! as they look: a dense vector of another size refused, a cosine vector
//! normalized when it is written, and alias actions applied one at a time.
//! A test can make it refuse the next call of a kind, or answer it without
//! its result.

mod collections;
mod points;
mod server;
mod state;

pub(super) use server::FakeQdrant;
