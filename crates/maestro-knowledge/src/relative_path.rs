//! Paths that a declaration or a manifest gives relative to a directory, which
//! they may not leave.

use serde::{Deserialize, Deserializer, de};
use std::path::{Path, PathBuf};

/// A path relative to a directory it cannot leave: `/` between its segments,
/// never absolute (a leading `/`) and never a `..` segment. A backslash and a
/// colon are refused too, in every segment: Windows reads a backslash as a
/// separator and a colon as a drive (`C:`, which replaces the directory the
/// path is joined to) or a file's data stream (`notes.md:draft`), where Linux
/// reads both as part of a name, and the path must mean the same on every
/// platform (ADR-0018).
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
    } else if text.contains(':') {
        Some("holds a colon, which Windows reads as a drive or a data stream")
    } else if text.starts_with('/') {
        Some("is absolute")
    } else if text.split('/').any(|segment| segment == "..") {
        Some("climbs out of its directory")
    } else {
        None
    }
}
