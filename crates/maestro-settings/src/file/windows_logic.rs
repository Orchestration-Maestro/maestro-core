//! Platform-neutral Windows decisions, tested on every host.

use std::io::{self, ErrorKind};

/// Turns a file result into an optional result: not found is absent.
pub(super) fn found<T>(result: io::Result<T>) -> io::Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if is_not_found(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Treats a directory-creation race as success.
pub(super) fn tolerate_existing(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(error) if already_exists(&error) => Ok(()),
        result => result,
    }
}

/// Whether a Windows file operation found no entry at its path.
fn is_not_found(error: &io::Error) -> bool {
    error.kind() == ErrorKind::NotFound
}

/// Whether a directory creation race already created the directory.
fn already_exists(error: &io::Error) -> bool {
    error.kind() == ErrorKind::AlreadyExists
}

/// Whether Windows reports a symbolic link or junction on the handle.
pub(super) fn is_reparse_point(attributes: u32, reparse_point: u32) -> bool {
    attributes & reparse_point != 0
}

#[cfg(test)]
mod tests {
    use super::{found, is_reparse_point, tolerate_existing};
    use std::io::{Error, ErrorKind};

    #[test]
    fn found_maps_not_found_to_none() {
        assert!(matches!(
            found::<()>(Err(Error::from(ErrorKind::NotFound))),
            Ok(None)
        ));
    }

    #[test]
    fn found_preserves_permission_denied() {
        assert!(matches!(
            found::<()>(Err(Error::from(ErrorKind::PermissionDenied))),
            Err(error) if error.kind() == ErrorKind::PermissionDenied
        ));
    }

    #[test]
    fn found_preserves_successful_values() {
        assert!(matches!(found(Ok(7)), Ok(Some(7))));
    }

    #[test]
    fn tolerate_existing_only_ignores_already_exists() {
        assert!(tolerate_existing(Err(Error::from(ErrorKind::AlreadyExists))).is_ok());
        assert!(matches!(
            tolerate_existing(Err(Error::from(ErrorKind::PermissionDenied))),
            Err(error) if error.kind() == ErrorKind::PermissionDenied
        ));
        assert!(tolerate_existing(Ok(())).is_ok());
    }

    #[test]
    fn detects_only_the_reparse_point_attribute() {
        const REPARSE_POINT: u32 = 0x0400;
        assert!(is_reparse_point(REPARSE_POINT, REPARSE_POINT));
        assert!(!is_reparse_point(0x0002, REPARSE_POINT));
    }
}
