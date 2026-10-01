//! The source checker's tests: the valid synthetic catalog and each of its
//! refused neighbours, the limit boundaries, hostile inputs, the frozen rows
//! and the filesystem adapter.

mod accepted;
mod area_packages;
mod area_regressions;
mod area_support;
mod bounds;
mod coverage;
mod directory;
mod extension;
mod git_boundary;
mod hostile;
mod layer_placements;
mod layout;
mod model_card;
mod placement_boundaries;
mod placement_guards;
mod qualified;
mod references;
mod registry;
mod root_boundaries;
mod rulings;
mod scan;
mod schema;
pub(crate) mod support;
mod yaml;
