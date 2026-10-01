//! The source checker's tests: the valid synthetic catalog and each of its
//! refused neighbours, the limit boundaries, hostile inputs, the frozen rows
//! and the filesystem adapter.

mod accepted;
mod area_support;
mod bounds;
mod coverage;
mod directory;
mod extension;
mod hostile;
mod layout;
mod model_card;
mod references;
mod registry;
mod rulings;
mod scan;
mod schema;
pub(crate) mod support;
mod yaml;
