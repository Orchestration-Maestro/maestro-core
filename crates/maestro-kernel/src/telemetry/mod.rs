//! Telemetry: pinned span names and component health (building block B11).
//!
//! Spans go through `tracing`. Their names, and the names of their
//! attributes, are pinned in [`span`], in one place, so that a rename is a
//! visible change (docs/architecture/05 §4). Nothing exports them in S1: the
//! OTLP exporter's crates build on prost-derive, which still needs syn 2 where
//! the workspace uses syn 3, and the dependency policy allows one version of
//! each crate. Export arrives with the first task that needs it, once
//! prost-derive has moved to syn 3, and the pinned names are what it will
//! carry. Until then a span reaches only the subscriber the process installs.
//!
//! [`Components::health`] reports each registered component as ready,
//! degraded or down, with its reason. A component whose check panics, gives
//! no answer in time or is still running from an earlier call is down, never
//! missing from the report. The journal, not telemetry, is the audit.

mod health;
pub mod span;
#[cfg(test)]
mod tests;

pub use health::{Components, DuplicateComponent, Report, Status};
