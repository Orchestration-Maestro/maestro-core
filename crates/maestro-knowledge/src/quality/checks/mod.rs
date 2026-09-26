//! The automatic checks of the quality gate (docs/architecture/01 §4), each
//! a rule ID with a documented threshold, run on a revision's canonical
//! document; the quality module's table lists them.

mod body;
mod flag;
mod page;
mod record;
mod rules;
mod secret;
mod text;

#[cfg(test)]
pub(super) use flag::Flag;
pub(super) use rules::run;
