//! Tests of the machine's checks, which `doctor` and `status` report: the
//! kernel's files, the services, each role's model card, and what doctor
//! finds but never touches.

mod findings;
mod kernel;
mod services;
mod support;
