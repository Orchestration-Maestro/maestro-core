//! Explicit replacement of a lost projection and frozen resume identity.

mod recovery;
mod run;

#[cfg(test)]
pub(super) use recovery::{
    ResumeCandidate, frozen_inputs, last_progress, recovery, recovery_inputs, select_resume,
    selected_inputs,
};
#[cfg(test)]
pub(super) use run::failure_message;
pub(super) use run::run;
