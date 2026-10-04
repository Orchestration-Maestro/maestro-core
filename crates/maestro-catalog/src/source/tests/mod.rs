//! The source checker's tests: the valid synthetic catalog and each of its
//! refused neighbours, the limit boundaries, hostile inputs, the frozen rows
//! and the filesystem adapter.

mod accepted;
mod area_ownership;
mod area_packages;
mod area_regressions;
mod area_support;
mod backend_extension_regressions;
mod backend_extensions;
mod backends;
mod bounds;
mod closure;
mod codeowners;
mod contracts;
mod contracts_eval;
mod coverage;
mod directory;
mod extension;
mod git_boundary;
mod hostile;
mod index;
mod layer_placements;
mod layout;
mod model_card;
mod owners;
mod owners_boundaries;
mod owners_rules;
mod ownership;
mod placement_boundaries;
mod placement_guards;
mod qualified;
mod references;
pub(crate) mod registry;
mod root_boundaries;
mod rulings;
mod scan;
mod schema;
mod secrets;
mod selection;
mod standards;
pub(crate) mod support;
mod versions;
mod yaml;

mod restrictive_standards;

mod quality_profile;

mod language;
