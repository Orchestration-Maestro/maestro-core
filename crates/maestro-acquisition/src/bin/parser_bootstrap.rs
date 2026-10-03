//! Single-threaded native parser namespace bootstrap; never a privileged helper.
#[cfg(target_os = "linux")]
use maestro_acquisition::isolation::{bootstrap_host, port::Refusal as Failure};
#[cfg(target_os = "linux")]
use std::io;

// Linux namespace controls cannot qualify other operating systems.
#[cfg(not(target_os = "linux"))]
mod platform;
#[cfg(not(target_os = "linux"))]
use platform::failure::Failure;

fn main() -> Result<(), Failure> {
    #[cfg(target_os = "linux")]
    {
        bootstrap_host::run(&mut io::stdout(), &mut io::stderr())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("Linux parser containment unsupported")
    }
}
