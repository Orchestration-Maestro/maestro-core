//! Safe Win32 file sharing and no-follow flags shared by held-handle adapters.
use std::ops::BitOr as _;
/// `FILE_SHARE_READ`: others may read the file while the handle is open.
const FILE_SHARE_READ: u32 = 0x0000_0001;
/// `FILE_SHARE_WRITE`: others may write the file while the handle is open.
const FILE_SHARE_WRITE: u32 = 0x0000_0002;
/// `FILE_SHARE_READ | FILE_SHARE_WRITE`: never deletion or renaming while held.
pub(super) const FILE_SHARE_READ_WRITE: u32 = 0x0000_0003;
/// `FILE_SHARE_DELETE`: others may delete or rename while the handle is open.
const FILE_SHARE_DELETE: u32 = 0x0000_0004;
/// All standard sharing bits, used only while the new child remains rollback-capable.
pub(super) fn file_share_all() -> u32 {
    FILE_SHARE_READ_WRITE.bitor(FILE_SHARE_DELETE)
}
/// `FILE_FLAG_BACKUP_SEMANTICS`: the open may name a directory.
pub(super) const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
/// `FILE_FLAG_OPEN_REPARSE_POINT`: a reparse point is opened itself, never followed.
pub(super) const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
/// `FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT`: a directory may open, and a
/// reparse point opens itself.
pub(super) const OPEN_REPARSE_DIRECTORY_FLAGS: u32 = 0x0220_0000;
// Each precombined value is exactly its named Win32 bits.
const _: () = assert!(FILE_SHARE_READ_WRITE == FILE_SHARE_READ | FILE_SHARE_WRITE);
const _: () = assert!(
    OPEN_REPARSE_DIRECTORY_FLAGS == FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT
);

#[cfg(test)]
mod tests {
    use super::{OPEN_REPARSE_DIRECTORY_FLAGS, file_share_all};
    use maestro_test_scratch::scratch_directory;
    use std::{fs, fs::OpenOptions, os::windows::fs::OpenOptionsExt as _};
    use windows_sys::Win32::{
        Foundation::{GENERIC_READ, GENERIC_WRITE},
        Storage::FileSystem::DELETE,
    };

    #[test]
    fn temporary_directory_shares_native_read_write_and_delete_access() {
        let root = scratch_directory().unwrap();
        let first = OpenOptions::new()
            .read(true)
            .share_mode(file_share_all())
            .custom_flags(OPEN_REPARSE_DIRECTORY_FLAGS)
            .open(&root)
            .unwrap();
        let second = OpenOptions::new()
            .access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
            .share_mode(file_share_all())
            .custom_flags(OPEN_REPARSE_DIRECTORY_FLAGS)
            .open(&root)
            .unwrap();
        drop((first, second));
        fs::remove_dir_all(root).unwrap();
    }
}
