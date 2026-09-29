mod assembly;
mod assembly_authority;
mod assembly_entry;
mod assembly_output;
mod assembly_versions;
mod budget;
mod conflict_emission;
mod conflict_structures;
mod conflicts;
mod features;
mod section_validation;
mod sections;
mod selection;
mod signals;
mod source;
mod spans;
mod support;
mod versions;

mod candidate_context;
mod parent_chain;
mod parent_tiers;

pub(super) use selection::{CandidateSource, candidate, prepared};
pub(super) use support::control;
