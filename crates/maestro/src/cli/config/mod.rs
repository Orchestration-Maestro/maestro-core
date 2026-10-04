//! `maestro config`: the settings, read, explained, changed and their
//! changes listed.

pub(in crate::cli) mod change;
mod editor;
#[cfg(test)]
mod editor_tests;
mod history;
mod show;
#[cfg(test)]
mod tests;

pub(super) use change::{Change, Places, run as change};
pub(super) use editor::run as editor;
pub(super) use history::run as history;
pub(super) use show::{explain, get, list};
