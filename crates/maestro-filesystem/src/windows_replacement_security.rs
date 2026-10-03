//! Pure comparison of validated Windows replacement security fields.
use std::io;

/// Compare validated descriptor fields after copying, without filesystem or Win32 effects.
pub(super) fn check_replacement_security(
    owner_matches: bool,
    acl_bytes: (&[u8], &[u8]),
    protection: (u32, u32),
) -> io::Result<()> {
    if !owner_matches || acl_bytes.0 != acl_bytes.1 || protection.0 != protection.1 {
        return Err(io::Error::other(
            "replacement would change the file's access protection; \
             its permissions or its folder's inheritable permissions differ; \
             fix them, then rerun",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check_replacement_security;
    use std::io::ErrorKind;
    use windows_sys::Win32::Security::{
        PROTECTED_DACL_SECURITY_INFORMATION, UNPROTECTED_DACL_SECURITY_INFORMATION,
    };

    #[test]
    fn replacement_security_comparator_checks_each_descriptor_field() {
        let acl = b"original ACL".as_slice();
        let protection = PROTECTED_DACL_SECURITY_INFORMATION;
        assert!(check_replacement_security(true, (acl, acl), (protection, protection)).is_ok());
        for (owner_matches, retained_acl, retained_protection) in [
            (false, acl, protection),
            (true, b"different ACL".as_slice(), protection),
            (true, acl, UNPROTECTED_DACL_SECURITY_INFORMATION),
        ] {
            let error = check_replacement_security(
                owner_matches,
                (acl, retained_acl),
                (protection, retained_protection),
            )
            .unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Other);
            assert!(error.to_string().contains("replacement would change"));
        }
    }
}
