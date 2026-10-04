//! Filesystem identity shared by authority admission and backup.

#[cfg(windows)]
use std::fs::File;
use std::{fs, io, path::Path};
#[cfg(windows)]
use winapi_util::file::information;

/// Reports whether a file has more than one filesystem link.
#[cfg_attr(
    unix,
    expect(
        clippy::unnecessary_wraps,
        reason = "Windows handle inspection can fail, so all targets share a fallible API"
    )
)]
pub(super) fn has_multiple_links(path: &Path, metadata: &fs::Metadata) -> io::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let _ = path;
        Ok(has_multiple_links_count(metadata.nlink()))
    }
    #[cfg(windows)]
    {
        let _ = metadata;
        let file = File::open(path)?;
        let information = information(file)?;
        Ok(has_multiple_links_count(information.number_of_links()))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, metadata);
        Ok(false)
    }
}

/// Reports whether `count` exceeds the one link owned by a standalone file.
fn has_multiple_links_count(count: u64) -> bool {
    count > 1
}
