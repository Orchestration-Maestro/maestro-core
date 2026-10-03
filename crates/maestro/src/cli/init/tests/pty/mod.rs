mod cases;
mod stream;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix::Pty;
#[cfg(windows)]
use windows::Pty;
