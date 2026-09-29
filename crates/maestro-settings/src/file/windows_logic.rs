//! Platform-neutral Windows decisions, tested on every host.

use std::io::{self, ErrorKind};

/// Whether a Windows file operation found no entry at its path.
pub(super) fn is_not_found(error: &io::Error) -> bool {
    error.kind() == ErrorKind::NotFound
}

/// Whether a directory creation race already created the directory.
pub(super) fn already_exists(error: &io::Error) -> bool {
    error.kind() == ErrorKind::AlreadyExists
}

/// Whether Windows reports a symbolic link or junction on the handle.
pub(super) fn is_reparse_point(attributes: u32, reparse_point: u32) -> bool {
    attributes & reparse_point != 0
}

#[cfg(test)]
mod tests {
    use super::{already_exists, is_not_found, is_reparse_point};
    use std::io::{Error, ErrorKind};

    #[test]
    fn classifies_windows_file_operation_errors() {
        assert!(is_not_found(&Error::from(ErrorKind::NotFound)));
        assert!(!is_not_found(&Error::from(ErrorKind::PermissionDenied)));
        assert!(already_exists(&Error::from(ErrorKind::AlreadyExists)));
        assert!(!already_exists(&Error::from(ErrorKind::PermissionDenied)));
    }

    #[test]
    fn detects_only_the_reparse_point_attribute() {
        const REPARSE_POINT: u32 = 0x0400;
        assert!(is_reparse_point(REPARSE_POINT, REPARSE_POINT));
        assert!(!is_reparse_point(0x0002, REPARSE_POINT));
    }
}
