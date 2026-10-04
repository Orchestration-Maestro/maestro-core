//! The sole unsafe boundary: security information belongs to an already-held handle.
#![allow(
    unsafe_code,
    reason = "the sole audited held-handle Win32 security boundary"
)]
#![deny(clippy::undocumented_unsafe_blocks)]
use crate::{
    windows_flags::{FILE_SHARE_READ_WRITE, OPEN_REPARSE_DIRECTORY_FLAGS},
    windows_replacement_security::check_replacement_security,
};
use std::{
    ffi::c_void,
    fs::{File, OpenOptions},
    io,
    os::windows::{
        fs::OpenOptionsExt as _,
        io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle},
    },
    path::Path,
    ptr,
};
use std::{ops::BitOr as _, slice};
use windows_sys::Win32::{
    Foundation::{GENERIC_ALL, GENERIC_WRITE, LocalFree},
    Security::{
        ACCESS_ALLOWED_ACE, ACE_HEADER, ACL,
        Authorization::{GetSecurityInfo, SE_FILE_OBJECT, SetSecurityInfo},
        CreateWellKnownSid, DACL_SECURITY_INFORMATION, EqualSid, GetAce,
        GetSecurityDescriptorControl, GetSecurityDescriptorDacl, GetSecurityDescriptorLength,
        GetSecurityDescriptorOwner, GetTokenInformation, INHERIT_ONLY_ACE, IsValidAcl, IsValidSid,
        OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSID, SE_DACL_PROTECTED,
        TOKEN_INFORMATION_CLASS, TOKEN_QUERY, TOKEN_USER, TokenUser,
        UNPROTECTED_DACL_SECURITY_INFORMATION, WELL_KNOWN_SID_TYPE, WinBuiltinAdministratorsSid,
        WinLocalSystemSid,
    },
    Storage::FileSystem::{
        DELETE, FILE_APPEND_DATA, FILE_DELETE_CHILD, FILE_DISPOSITION_INFO, FILE_ID_INFO,
        FILE_WRITE_ATTRIBUTES, FILE_WRITE_DATA, FILE_WRITE_EA, FileDispositionInfo, FileIdInfo,
        GetFileInformationByHandleEx, SetFileInformationByHandle, WRITE_DAC, WRITE_OWNER,
    },
    System::{
        SystemServices::{
            ACCESS_ALLOWED_ACE_TYPE, ACCESS_DENIED_ACE_TYPE, ACCESS_DENIED_CALLBACK_ACE_TYPE,
            ACCESS_DENIED_CALLBACK_OBJECT_ACE_TYPE, ACCESS_DENIED_OBJECT_ACE_TYPE,
        },
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
};

/// ReFS-compatible identity: full volume serial and 128-bit file ID, never a 64-bit fallback.
type FileIdentity = (u64, [u8; 16]);

/// The shared equality check for original names and canonical spellings.
pub(super) fn same_file(left: &File, right: &File) -> io::Result<bool> {
    Ok(file_identity(left)? == file_identity(right)?)
}

/// Open the created directory for native deletion and compare its full held identity.
/// No reparse point is followed; the API refuses a non-empty directory.
pub(super) fn remove_created_directory(path: &Path, created: &File) -> io::Result<()> {
    let file = OpenOptions::new()
        .access_mode(DELETE)
        .share_mode(FILE_SHARE_READ_WRITE)
        .custom_flags(OPEN_REPARSE_DIRECTORY_FLAGS)
        .open(path)?;
    if !same_file(&file, created)? {
        return Err(io::Error::other(
            "rollback failed: created directory changed",
        ));
    }
    let mut disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    let bytes = u32::try_from(size_of::<FILE_DISPOSITION_INFO>()).map_err(io::Error::other)?;
    // SAFETY: file owns a live DELETE-capable no-follow directory handle; disposition is
    // aligned initialized storage of exactly bytes, exclusively borrowed for this call.
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            ptr::from_mut(&mut disposition).cast(),
            bytes,
        )
    } == 0
    {
        return Err(io::Error::other(format!(
            "rollback failed: {}",
            io::Error::last_os_error()
        )));
    }
    Ok(())
}

/// Query identity from a live held handle, using the already-enabled Win32 filesystem API.
fn file_identity(file: &File) -> io::Result<FileIdentity> {
    file_identity_with(file, |file, information| {
        let bytes = u32::try_from(size_of::<FILE_ID_INFO>()).map_err(io::Error::other)?;
        // SAFETY: file owns a live handle; information is aligned initialized FILE_ID_INFO
        // storage of exactly bytes, alive and exclusively borrowed throughout this call.
        Ok(unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FileIdInfo,
                ptr::from_mut(information).cast(),
                bytes,
            )
        })
    })
}

/// Private query seam tests an unsupported/failing API without a production override.
pub(super) fn file_identity_with(
    file: &File,
    query: impl FnOnce(&File, &mut FILE_ID_INFO) -> io::Result<i32>,
) -> io::Result<FileIdentity> {
    let mut information = FILE_ID_INFO::default();
    if query(file, &mut information)? == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((
        information.VolumeSerialNumber,
        information.FileId.Identifier,
    ))
}

/// Rust-owned self-relative security descriptor, retaining its embedded SID and ACL pointers.
struct SecurityDescriptor {
    /// Initialized descriptor bytes returned by `GetSecurityInfo`.
    memory: Vec<u8>,
    /// Owner SID within memory.
    owner: PSID,
    /// DACL within memory, possibly NULL.
    dacl: *mut ACL,
}
impl SecurityDescriptor {
    /// Protected versus inheritable DACL state, independent of ACL bytes.
    fn protection(&self) -> io::Result<u32> {
        let mut control = 0;
        let mut revision = 0;
        // SAFETY: GetSecurityInfo initialized the descriptor, retained throughout this call.
        if unsafe {
            GetSecurityDescriptorControl(
                self.memory.as_ptr().cast_mut().cast(),
                &raw mut control,
                &raw mut revision,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(if control & SE_DACL_PROTECTED != 0 {
            PROTECTED_DACL_SECURITY_INFORMATION
        } else {
            UNPROTECTED_DACL_SECURITY_INFORMATION
        })
    }

    /// Validate the API-owned ACL before borrowing its complete byte representation.
    fn acl_bytes(&self) -> io::Result<&[u8]> {
        if self.dacl.is_null() {
            return Ok(&[]);
        }
        // SAFETY: dacl points into this live GetSecurityInfo descriptor.
        if unsafe { IsValidAcl(self.dacl) } == 0 {
            return Err(io::Error::other("unverifiable DACL"));
        }
        // SAFETY: IsValidAcl validated its header/size; self retains the owning allocation.
        let size = usize::from(unsafe { (*self.dacl).AclSize });
        // SAFETY: the validated ACL is size bytes long and remains owned by self.
        Ok(unsafe { slice::from_raw_parts(self.dacl.cast(), size) })
    }

    /// Read through the matching high-level API, then immediately own and free its bytes.
    fn read(file: &File) -> io::Result<Self> {
        let information = OWNER_SECURITY_INFORMATION.bitor(DACL_SECURITY_INFORMATION);
        let mut allocated = ptr::null_mut();
        // SAFETY: live held handle; writable descriptor slot; optional component slots omitted.
        let result = unsafe {
            GetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                information,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &raw mut allocated,
            )
        };
        if result != 0 {
            return Err(io::Error::from_raw_os_error(result.cast_signed()));
        }
        // SAFETY: GetSecurityInfo returned a valid self-relative descriptor and its allocation.
        let length = unsafe { GetSecurityDescriptorLength(allocated) } as usize;
        // SAFETY: initialized descriptor owns length readable bytes; copy before freeing it.
        let memory = unsafe { slice::from_raw_parts(allocated.cast::<u8>(), length) }.to_vec();
        // SAFETY: GetSecurityInfo allocated with LocalAlloc; copied fully and freed once here.
        unsafe {
            LocalFree(allocated);
        }
        let mut security = Self {
            memory,
            owner: ptr::null_mut(),
            dacl: ptr::null_mut(),
        };
        let descriptor = security.memory.as_mut_ptr().cast();
        let mut defaulted = 0;
        let mut present = 0;
        // SAFETY: initialized self-relative descriptor stays alive; writable output slots.
        if unsafe {
            GetSecurityDescriptorOwner(descriptor, &raw mut security.owner, &raw mut defaulted)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the same owned descriptor and writable output slots remain alive.
        if unsafe {
            GetSecurityDescriptorDacl(
                descriptor,
                &raw mut present,
                &raw mut security.dacl,
                &raw mut defaulted,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(security)
    }
}

/// Retain the original owner and exact DACL protection before staged contents are written.
/// Both handles are held and the destination has `WRITE_OWNER`/`WRITE_DAC` access.
pub(super) fn retain_replacement_security(original: &File, staged: &File) -> io::Result<()> {
    let descriptor = SecurityDescriptor::read(original)?;
    let protection = descriptor.protection()?;
    // SAFETY: staged is live and security-capable; owner/dacl point into the retained descriptor.
    let result = unsafe {
        SetSecurityInfo(
            staged.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION
                .bitor(DACL_SECURITY_INFORMATION)
                .bitor(protection),
            descriptor.owner,
            ptr::null_mut(),
            descriptor.dacl,
            ptr::null(),
        )
    };
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result.cast_signed()));
    }
    let retained = SecurityDescriptor::read(staged)?;
    check_replacement_security(
        equal_sid(descriptor.owner, retained.owner),
        (descriptor.acl_bytes()?, retained.acl_bytes()?),
        (protection, retained.protection()?),
    )
}

/// Check owner and DACL on a held file or directory, never by name.
/// Owner must be the token user. Write ACEs may name that user, SYSTEM or Administrators,
/// the privileged-write exceptions used by OpenSSH for Windows' secure-file rule:
/// See the Win32-OpenSSH wiki, "Security protection of various files in win32 openssh".
/// NULL DACLs and unsupported ACE shapes refuse rather than guessing effective access.
pub(super) fn private_metadata(file: &File) -> io::Result<()> {
    let descriptor = SecurityDescriptor::read(file)?;
    let user = token_user()?;
    // SAFETY: token_user returns aligned initialized TOKEN_USER data with its embedded SID alive.
    let user_sid = unsafe { (*(user.as_ptr().cast::<TOKEN_USER>())).User.Sid };
    if !equal_sid(descriptor.owner, user_sid) {
        return Err(io::Error::other("foreign-owned preferences"));
    }
    if descriptor.dacl.is_null() {
        return Err(io::Error::other("NULL DACL permits other writers"));
    }
    let system = well_known_sid(WinLocalSystemSid)?;
    let administrators = well_known_sid(WinBuiltinAdministratorsSid)?;
    check_acl(
        descriptor.dacl,
        &[
            user_sid,
            system.as_ptr().cast_mut().cast(),
            administrators.as_ptr().cast_mut().cast(),
        ],
    )
}

/// The process token's user information, in an aligned buffer owning its SID.
fn token_user() -> io::Result<Vec<u64>> {
    let mut handle = ptr::null_mut();
    // SAFETY: the pseudo-process handle is valid; output is a writable handle slot.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut handle) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful OpenProcessToken transfers a unique handle closed by OwnedHandle.
    let token = unsafe { OwnedHandle::from_raw_handle(handle) };
    token_information(&token, TokenUser).map(|(buffer, _)| buffer)
}

/// Read one token information class into aligned owned storage, returning its initialized size.
pub(super) fn token_information(
    token: &OwnedHandle,
    kind: TOKEN_INFORMATION_CLASS,
) -> io::Result<(Vec<u64>, u32)> {
    let mut needed = 0;
    // SAFETY: this sizing call writes only the length; no buffer is supplied.
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            kind,
            ptr::null_mut(),
            0,
            &raw mut needed,
        );
    }
    if needed == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
    // SAFETY: buffer is sufficiently sized/aligned and remains alive with its embedded SID.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            kind,
            buffer.as_mut_ptr().cast(),
            needed,
            &raw mut needed,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok((buffer, needed))
}

/// A Windows well-known SID, with aligned storage large enough for `SECURITY_MAX_SID_SIZE`.
fn well_known_sid(kind: WELL_KNOWN_SID_TYPE) -> io::Result<[u32; 17]> {
    let mut sid = [0u32; 17];
    let mut bytes = 68;
    // SAFETY: sid provides 68 aligned writable bytes, more than the 68-byte SID maximum.
    if unsafe {
        CreateWellKnownSid(
            kind,
            ptr::null_mut(),
            sid.as_mut_ptr().cast(),
            &raw mut bytes,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(sid)
}

/// Compare only non-null SIDs validated by Windows; their owning buffers remain alive.
fn equal_sid(left: PSID, right: PSID) -> bool {
    // SAFETY: inputs are pointers into live API-owned descriptors/token buffers/validated ACEs.
    unsafe { IsValidSid(left) != 0 && IsValidSid(right) != 0 && EqualSid(left, right) != 0 }
}

/// Refuse any effective allow ACE granting mutation to an untrusted principal.
fn check_acl(dacl: *mut ACL, writers: &[PSID]) -> io::Result<()> {
    // SAFETY: dacl points into the live GetSecurityInfo descriptor; validate before traversal.
    if unsafe { IsValidAcl(dacl) } == 0 {
        return Err(io::Error::other("unverifiable DACL"));
    }
    // SAFETY: IsValidAcl checked the ACL header and size in the live descriptor.
    let count = unsafe { (*dacl).AceCount };
    for index in 0..u32::from(count) {
        let mut ace = ptr::null_mut();
        // SAFETY: index is below AceCount of the validated ACL; output is a writable pointer.
        if unsafe { GetAce(dacl, index, &raw mut ace) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: GetAce returned a header within the validated, still-live ACL.
        let header = unsafe { &*ace.cast::<ACE_HEADER>() };
        if u32::from(header.AceFlags) & INHERIT_ONLY_ACE != 0 {
            continue;
        }
        match u32::from(header.AceType) {
            ACCESS_DENIED_ACE_TYPE
            | ACCESS_DENIED_OBJECT_ACE_TYPE
            | ACCESS_DENIED_CALLBACK_ACE_TYPE
            | ACCESS_DENIED_CALLBACK_OBJECT_ACE_TYPE => {}
            ACCESS_ALLOWED_ACE_TYPE => check_allow(ace, writers)?,
            _ => return Err(io::Error::other("unverifiable DACL ACE type")),
        }
    }
    Ok(())
}

/// Inspect the mask and inline SID of a standard validated allow ACE.
fn check_allow(ace: *mut c_void, writers: &[PSID]) -> io::Result<()> {
    // SAFETY: GetAce returned the header of an ACE in a validated live ACL.
    let size = usize::from(unsafe { (*ace.cast::<ACE_HEADER>()).AceSize });
    if size < 16 {
        return Err(io::Error::other("unverifiable allow ACE size"));
    }
    // SAFETY: the ACE has at least 16 bytes; its inline SID count byte is at offset 9.
    let count = usize::from(unsafe { *ace.cast::<u8>().add(9) });
    if 16 + 4 * count > size {
        return Err(io::Error::other("unverifiable allow ACE SID"));
    }
    // SAFETY: standard allow ACE type, minimum struct size and entire inline SID are checked.
    let allowed = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
    let write = FILE_WRITE_DATA
        .bitor(FILE_APPEND_DATA)
        .bitor(FILE_WRITE_EA)
        .bitor(FILE_WRITE_ATTRIBUTES)
        .bitor(FILE_DELETE_CHILD)
        .bitor(DELETE)
        .bitor(WRITE_DAC)
        .bitor(WRITE_OWNER)
        .bitor(GENERIC_WRITE)
        .bitor(GENERIC_ALL);
    let sid = ptr::addr_of!(allowed.SidStart).cast_mut().cast();
    if allowed.Mask & write != 0 && !writers.iter().any(|writer| equal_sid(sid, *writer)) {
        return Err(io::Error::other("other-writable preferences"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows_security_fixture_tests::{icacls, native_dacl};
    use maestro_test_scratch::scratch_directory;
    use std::{env, fs};

    #[test]
    fn descriptor_fields_match_native_owner_dacl_and_protection() {
        let root = scratch_directory().unwrap();
        let path = root.join("file");
        fs::write(&path, b"fixture").unwrap();
        let account = env::var("USERNAME").unwrap();
        for (inheritance, protection) in [
            ("/inheritance:e", UNPROTECTED_DACL_SECURITY_INFORMATION),
            ("/inheritance:r", PROTECTED_DACL_SECURITY_INFORMATION),
        ] {
            icacls(&path, &["/setowner", &account]);
            icacls(&path, &[inheritance, "/grant:r", &format!("{account}:(F)")]);
            let file = File::open(&path).unwrap();
            let descriptor = SecurityDescriptor::read(&file).unwrap();
            let user = token_user().unwrap();
            assert!(user.len() >= size_of::<TOKEN_USER>().div_ceil(size_of::<u64>()));
            // SAFETY: initialized aligned TOKEN_USER owns its SID until user is dropped.
            let user_sid = unsafe { (*user.as_ptr().cast::<TOKEN_USER>()).User.Sid };
            assert!(equal_sid(descriptor.owner, user_sid));
            assert_eq!(descriptor.protection().unwrap(), protection);
            let expected = native_dacl(&path);
            assert!(!expected.is_empty());
            assert_eq!(descriptor.acl_bytes().unwrap(), expected);
            if protection == PROTECTED_DACL_SECURITY_INFORMATION {
                private_metadata(&file).unwrap();
            }
        }
        icacls(&path, &["/grant", "*S-1-1-0:(W)"]);
        assert!(private_metadata(&File::open(&path).unwrap()).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn native_acl_masks_and_shape_boundaries_refuse_other_writers() {
        use crate::windows_test_security::with_allow_acl;
        use windows_sys::Win32::Security::WinWorldSid;
        let sid = well_known_sid(WinWorldSid).unwrap();
        let trusted = well_known_sid(WinLocalSystemSid).unwrap();
        let world = sid.as_ptr().cast_mut().cast();
        let writers = [trusted.as_ptr().cast_mut().cast()];
        for mask in [
            FILE_WRITE_DATA,
            FILE_APPEND_DATA,
            FILE_WRITE_EA,
            FILE_WRITE_ATTRIBUTES,
            FILE_DELETE_CHILD,
            DELETE,
            WRITE_DAC,
            WRITE_OWNER,
            GENERIC_WRITE,
            GENERIC_ALL,
        ] {
            with_allow_acl(world, mask, |acl| {
                assert!(check_acl(acl, &writers).is_err(), "write mask {mask:x}");
                assert!(check_acl(acl, &[world]).is_ok());
            });
        }
    }

    #[test]
    fn native_acl_shape_boundaries_use_validated_native_storage() {
        use crate::windows_test_security::probe_acl_shapes;
        use windows_sys::Win32::Security::WinWorldSid;
        let sid = well_known_sid(WinWorldSid).unwrap();
        let trusted = well_known_sid(WinLocalSystemSid).unwrap();
        let world = sid.as_ptr().cast_mut().cast();
        let writers = [trusted.as_ptr().cast_mut().cast()];
        probe_acl_shapes(
            world,
            |acl, accepted| assert_eq!(check_acl(acl, &writers).is_ok(), accepted),
            |ace, message, world_writer| {
                let writers = if world_writer { [world] } else { writers };
                let result = check_allow(ace, &writers);
                if message.is_empty() {
                    assert!(result.is_ok());
                } else {
                    assert!(result.unwrap_err().to_string().contains(message));
                }
            },
        );
    }

    #[test]
    fn native_sid_comparison_refuses_null_and_invalid_sids() {
        let sid = well_known_sid(WinLocalSystemSid).unwrap();
        let invalid = [0u32; 17];
        let valid = sid.as_ptr().cast_mut().cast();
        let invalid = invalid.as_ptr().cast_mut().cast();
        assert!(!equal_sid(valid, invalid));
        assert!(!equal_sid(invalid, valid));
        assert!(!equal_sid(ptr::null_mut(), valid));
        assert!(!equal_sid(valid, ptr::null_mut()));
    }
}
