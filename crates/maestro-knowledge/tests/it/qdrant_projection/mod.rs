//! The Qdrant projection (T026): a complete chunk set published as a search
//! generation in its own Qdrant collection, behind its collection's alias,
//! against the fake Qdrant always and a real one when `MAESTRO_QDRANT_URL`
//! names it (see `backends.rs`): the collection and what each point holds,
//! batches refused whole, builds resumed from their journaled progress, the
//! alias moved only past the checks, and the refusals before any work.
#![cfg(test)]

mod alias_moves;
mod backends;
mod built_generations;
mod fake;
mod kernel;
mod models;
mod projection_rebuild;
mod publication_verification;
mod refused_batches;
mod resumed_builds;
mod scratch_cleanup;
mod search_routes;
mod server_version;
mod stopped_builds;
mod support;

pub(crate) use backends::synthetic_fake_qdrant_url;
