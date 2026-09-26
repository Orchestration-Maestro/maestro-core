//! `maestro setup` (plan D14, FR-S1-015): the search service Maestro needs,
//! Qdrant 1.19.1 pinned by digest, installed under the kernel's data
//! directory as a systemd user unit, bound to 127.0.0.1 with telemetry off.
//! Without `--yes` it previews and changes nothing; a second run with
//! everything in place changes nothing. It installs on Linux on x86-64, the
//! reference workstation's platform, and prints the manual steps elsewhere;
//! where no systemd user manager runs, it is refused before any step.

mod command;
mod release;
mod service;
#[cfg(test)]
mod tests;
mod tools;

pub(super) use command::{Readiness, readiness, run};
pub(super) use release::{HOST, HTTP_PORT, QDRANT, SERVICE};
#[cfg(test)]
pub(super) use service::Step;
