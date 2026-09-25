//! Filesystem access that never follows a link below the root its caller names, which resolves
//! once, one interface over two platforms.
//! rustix's directory-relative calls serve Unix and the standard library's Win32 open flags serve
//! Windows, so neither the store nor the tokenizer branches on the platform (ADR-0018).
mod root;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub(crate) use unix::{Directory, open_nofollow};
#[cfg(windows)]
pub(crate) use windows::{Directory, open_nofollow};
