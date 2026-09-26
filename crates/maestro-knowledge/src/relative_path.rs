//! Paths that a declaration or a manifest gives relative to a directory, which
//! they may not leave.

use serde::{Deserialize, Deserializer, de};
use std::path::{Path, PathBuf};

/// A path relative to a directory it cannot leave: `/` between its segments,
/// never absolute (a leading `/`, or a Windows drive such as `C:`) and never a
/// `..` segment. A backslash is refused too, since Windows reads it as a
/// separator and Linux as part of a name, and the path must mean the same on
/// every platform (ADR-0018).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelativePath(String);

impl RelativePath {
    /// The path as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The path under `directory`, joined one segment at a time.
    #[must_use]
    pub fn under(&self, directory: &Path) -> PathBuf {
        self.0
            .split('/')
            .fold(directory.to_path_buf(), |path, segment| path.join(segment))
    }
}

impl<'de> Deserialize<'de> for RelativePath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        match refusal(&text) {
            Some(reason) => Err(de::Error::custom(format_args!(
                "the path {text:?} {reason}"
            ))),
            None => Ok(Self(text)),
        }
    }
}

/// Why `text` is not a path that stays inside its directory, when it is not.
fn refusal(text: &str) -> Option<&'static str> {
    if text.is_empty() {
        Some("is empty")
    } else if text.contains('\\') {
        Some("holds a backslash, where `/` separates segments")
    } else if text.starts_with('/') || starts_with_drive(text) {
        Some("is absolute")
    } else if text.split('/').any(|segment| segment == "..") {
        Some("climbs out of its directory")
    } else {
        None
    }
}

/// Whether `text` starts with a Windows drive, such as `C:`.
fn starts_with_drive(text: &str) -> bool {
    let mut characters = text.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && characters.next() == Some(':')
}
