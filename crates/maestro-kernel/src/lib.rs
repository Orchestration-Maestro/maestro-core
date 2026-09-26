//! The kernel of Maestro: the single authoritative store every later
//! capability composes (docs/architecture/04 §3, building blocks B1–B11).
//!
//! It starts with the content-addressed artifact store (building block B3) and
//! the directory the kernel keeps its data in; the database, journal, scopes
//! and jobs join it slice by slice.

pub mod artifact;
pub mod binding;
pub mod capability;
mod filesystem;
pub mod gateway;
pub mod paths;
pub mod store;
pub mod telemetry;
