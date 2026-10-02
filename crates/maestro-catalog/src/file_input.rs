//! Plain owned-file inputs contain data, never a filesystem capability.
/// One intended file and its bytes at preview time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileInput {
    /// Root-relative, slash-separated file name.
    pub path: String,
    /// The complete bytes to write.
    pub bytes: Vec<u8>,
}

impl FileInput {
    /// Construct a file input for a root-relative path.
    pub fn new(path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            bytes: bytes.into(),
        }
    }
}
