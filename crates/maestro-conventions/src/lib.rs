//! Helpers for repository policy checks and private-content refusal.

mod conventions;
pub mod privacy;

pub use conventions::{
    broken_links, counted_lines, names_a_personal_directory, repository_files, root,
};
