//! Existing-leaf replacement stays inside the shared held-handle write floor.
use super::paths::AuthorizedPath;
use std::io;

impl AuthorizedPath<'_> {
    /// Replace exactly the previewed bytes, retaining this path's authority and parent handle.
    /// As with native rename, a same-user writer can race the last comparison and swap.
    ///
    /// # Errors
    /// Refuses read authority, changed policy/parents, links, changed bytes and I/O failures.
    pub fn replace_verified(&self, expected: &[u8], replacement: &[u8]) -> io::Result<()> {
        self.prepare_creation()?;
        self.parent
            .replace_verified(&self.name, expected, replacement, || {
                self.prepare_creation()
            })?;
        self.check_current_policy()
    }
}
