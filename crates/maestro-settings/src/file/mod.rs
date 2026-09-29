//! A preferences file's own I/O: its directory opened without following a
//! link below the directory its caller trusts, a lock that serializes the
//! edits of every process, and a replacement that detects another
//! program's change, keeps the file's permissions and can be undone.

mod bounded;
mod edit;
mod layers;
mod place;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(any(windows, test))]
mod windows_logic;

pub use edit::{FileEdit, FileError, RestoreError};
pub use layers::FileLayers;
pub use place::FilePlace;
