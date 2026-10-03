//! `catalog`: the catalog's authoring commands.

mod check;
mod codeowners;
pub(super) mod compile;
pub(super) mod dispatch;
pub(super) mod index;
mod owners;
pub(in crate::cli) mod project;

pub(super) use check::today;
