//! Single-threaded native parser namespace bootstrap; never a privileged helper.
#[cfg(target_os = "linux")]
use maestro_acquisition::isolation::{bootstrap, port::Refusal};
#[cfg(target_os = "linux")]
use std::io;

#[cfg(target_os = "linux")]
fn main() -> Result<(), Refusal> {
    bootstrap::run(&mut io::stdout(), &mut io::stderr())
}
// Linux namespace controls cannot qualify other operating systems.
#[cfg(not(target_os = "linux"))]
fn main() -> Result<(), &'static str> {
    Err("Linux parser containment unsupported")
}
