//! Tests of setup: the pinned release and the platforms it installs on, on
//! every host; the layout, the unit, the user manager it needs and the
//! install itself on Unix hosts, where a unit's paths are written as setup
//! writes them on Linux and the fake tools are shell scripts.

#[cfg(unix)]
mod install;
#[cfg(unix)]
mod manager;
mod platform;
#[cfg(unix)]
pub(in crate::cli) mod support;
#[cfg(unix)]
mod unit;
