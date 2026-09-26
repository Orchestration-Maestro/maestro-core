//! How the tests of `doctor` and `status` run the binary: the model router
//! looked for where nothing answers, and, on Unix, the fake tools first on
//! the `PATH`, so that no test reaches the user's own router or systemd
//! manager.

#[cfg(unix)]
use super::fakes::Fakes;
use super::support::{Ended, Home, Running};
use std::net::TcpListener;

/// An address on the loopback that nothing answers at: a port just freed.
pub(crate) fn nothing_at() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("http://{}", listener.local_addr().unwrap())
}

/// Runs the binary with `arguments` in `home`, the router looked for at
/// `router`.
pub(crate) fn checked_with(home: &Home, router: &str, arguments: &[&str]) -> Ended {
    let mut command = home.command(arguments);
    command.env("MAESTRO_ROUTER_URL", router);
    #[cfg(unix)]
    command.env("PATH", Fakes::in_home(home).path());
    Running::of(command).finish()
}

/// Runs the binary with `arguments` in `home`, the router looked for where
/// nothing answers.
pub(crate) fn checked(home: &Home, arguments: &[&str]) -> Ended {
    checked_with(home, &nothing_at(), arguments)
}
