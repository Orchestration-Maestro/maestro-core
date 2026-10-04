//! Filesystem access that never follows a link below the root its caller names, which resolves
//! once, one interface over two platforms.
//! rustix's directory-relative calls serve Unix and the standard library's Win32 open flags serve
//! Windows, so neither the store nor the tokenizer branches on the platform (ADR-0018).
#[cfg(test)]
mod bounded_tests;
#[cfg(test)]
mod canonical_identity_tests;
#[cfg(all(test, windows))]
mod canonical_windows_identity_tests;
#[cfg(test)]
mod created_identity_tests;
mod listing;
#[cfg(test)]
mod listing_tests;
mod publication;
mod read;
mod replacement;
#[cfg(test)]
mod replacement_tests;
mod root;
#[cfg(test)]
mod tests;
#[cfg(unix)]
mod unix;
#[cfg(unix)]
mod unix_creation;
#[cfg(all(test, unix))]
mod unix_mutation_tests;
#[cfg(windows)]
mod windows;
#[cfg(all(test, windows))]
mod windows_behavior_tests;
#[cfg(windows)]
mod windows_flags;
#[cfg(windows)]
mod windows_replacement_security;
#[cfg(windows)]
mod windows_security;
#[cfg(all(test, windows))]
mod windows_security_fixture_tests;
#[cfg(all(test, windows))]
mod windows_test_security;
#[cfg(unix)]
pub use unix::{Directory, open_nofollow};
#[cfg(windows)]
pub use windows::{Directory, open_nofollow};

pub use listing::{Entry, EntryKind};

pub use publication::PublicationChecks;
