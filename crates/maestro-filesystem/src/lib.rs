//! Filesystem access that never follows a link below the root its caller names, which resolves
//! once, one interface over two platforms.
//! rustix's directory-relative calls serve Unix and the standard library's Win32 open flags serve
//! Windows, so neither the store nor the tokenizer branches on the platform (ADR-0018).
#[cfg(test)]
mod bounded_tests;
mod listing;
#[cfg(test)]
mod listing_tests;
mod read;
mod root;
#[cfg(test)]
mod tests;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
mod windows_security;
#[cfg(unix)]
pub use unix::{Directory, open_nofollow};
#[cfg(windows)]
pub use windows::{Directory, open_nofollow};

pub use listing::{Entry, EntryKind};
