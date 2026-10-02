//! Complete-record publication keeps its temporary source open through identity-checked rollback.
#[cfg(unix)]
use crate::unix::Directory;
#[cfg(windows)]
use crate::windows::Directory;
use std::io::{self, Read as _};

/// Named checkpoints supplied by the trusted effect adapter, never by file content.
#[derive(Debug)]
pub struct PublicationChecks<O, B, A> {
    /// Recheck the source after its open and before reading any bytes.
    pub after_source_open: O,
    /// Recheck both source and destination immediately before the hard link.
    pub before_link: B,
    /// Recheck both paths after publication, while identity rollback is still possible.
    pub after_link: A,
}

impl Directory {
    /// Publish a regular sibling without replacement, rechecking policy and held identity.
    /// Reads stay bound to the verified handle. Held Windows directories block parent renames;
    /// regular names may be deleted or renamed, but publication detects object substitution.
    ///
    /// # Errors
    /// Refuses policy changes, source/target replacements, links and failed verified rollback.
    pub fn publish_verified(
        &self,
        from: &str,
        to: &str,
        expected: &[u8],
        checks: PublicationChecks<
            impl FnOnce() -> io::Result<()>,
            impl FnOnce() -> io::Result<()>,
            impl FnOnce() -> io::Result<()>,
        >,
    ) -> io::Result<()> {
        self.publish_verified_inner((from, to), expected, checks, || {})
    }

    /// Shared publication with a private scheduling seam after the final source comparison.
    fn publish_verified_inner(
        &self,
        (from, to): (&str, &str),
        expected: &[u8],
        checks: PublicationChecks<
            impl FnOnce() -> io::Result<()>,
            impl FnOnce() -> io::Result<()>,
            impl FnOnce() -> io::Result<()>,
        >,
        after_compare: impl FnOnce(),
    ) -> io::Result<()> {
        let mut source = self.open_regular(from)?;
        (checks.after_source_open)()?;
        self.verify_created(from, &source)?;
        let mut bytes = Vec::new();
        source.read_to_end(&mut bytes)?;
        if bytes != expected {
            return Err(io::Error::other(
                "publication refused: source bytes changed",
            ));
        }
        (checks.before_link)()?;
        self.verify_created(from, &source)?;
        after_compare();
        self.link(from, to)?;
        if let Err(error) = (checks.after_link)().and_then(|()| self.verify_created(to, &source)) {
            self.remove_created_bytes(to, &source, &bytes)
                .map_err(|cleanup| {
                    io::Error::other(format!("{error}; rollback failed: {cleanup}"))
                })?;
            return Err(error);
        }
        Ok(())
    }
    /// Test-only race between the last source comparison and the actual link syscall.
    #[cfg(test)]
    pub(crate) fn publish_after_compare_for_test(
        &self,
        from: &str,
        to: &str,
        expected: &[u8],
        after_compare: impl FnOnce(),
    ) -> io::Result<()> {
        self.publish_verified_inner(
            (from, to),
            expected,
            PublicationChecks {
                after_source_open: || Ok(()),
                before_link: || Ok(()),
                after_link: || Ok(()),
            },
            after_compare,
        )
    }
}
