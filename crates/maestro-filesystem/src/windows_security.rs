//! The sole unsafe boundary: security information belongs to an already-held handle.
#![allow(
    unsafe_code,
    reason = "the sole audited held-handle Win32 security boundary"
)]
#![deny(clippy::undocumented_unsafe_blocks)]
use std::{
    ffi::c_void,
    fs::File,
    io,
    os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{GENERIC_ALL, GENERIC_WRITE, LocalFree},
    Security::{
        ACCESS_ALLOWED_ACE, ACE_HEADER, ACL,
        Authorization::{GetSecurityInfo, SE_FILE_OBJECT},
        CreateWellKnownSid, DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetTokenInformation,
        INHERIT_ONLY_ACE, IsValidAcl, IsValidSid, OWNER_SECURITY_INFORMATION, PSID, TOKEN_QUERY,
        TOKEN_USER, TokenUser, WELL_KNOWN_SID_TYPE, WinBuiltinAdministratorsSid, WinLocalSystemSid,
    },
    Storage::FileSystem::{
        DELETE, FILE_APPEND_DATA, FILE_DELETE_CHILD, FILE_WRITE_ATTRIBUTES, FILE_WRITE_DATA,
        FILE_WRITE_EA, WRITE_DAC, WRITE_OWNER,
    },
    System::{
        SystemServices::{
            ACCESS_ALLOWED_ACE_TYPE, ACCESS_DENIED_ACE_TYPE, ACCESS_DENIED_CALLBACK_ACE_TYPE,
            ACCESS_DENIED_CALLBACK_OBJECT_ACE_TYPE, ACCESS_DENIED_OBJECT_ACE_TYPE,
        },
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
};

/// Memory allocated by `GetSecurityInfo`, which owns the returned SID and ACL pointers.
struct SecurityDescriptor(*mut c_void);
impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        // SAFETY: GetSecurityInfo allocated this descriptor with LocalAlloc; freed once here.
        unsafe {
            LocalFree(self.0);
        }
    }
}

/// Check owner and DACL on a held file or directory, never by name.
/// Owner must be the token user. Write ACEs may name that user, SYSTEM or Administrators,
/// the privileged-write exceptions used by OpenSSH for Windows' secure-file rule:
/// See the Win32-OpenSSH wiki, "Security protection of various files in win32 openssh".
/// NULL DACLs and unsupported ACE shapes refuse rather than guessing effective access.
pub(super) fn private_metadata(file: &File) -> io::Result<()> {
    let mut owner = ptr::null_mut();
    let mut dacl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    // SAFETY: file owns a live handle; output pointers refer to writable stack slots.
    let result = unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &raw mut owner,
            ptr::null_mut(),
            &raw mut dacl,
            ptr::null_mut(),
            &raw mut descriptor,
        )
    };
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result.cast_signed()));
    }
    let _descriptor = SecurityDescriptor(descriptor);
    let user = token_user()?;
    // SAFETY: token_user returns aligned initialized TOKEN_USER data with its embedded SID alive.
    let user_sid = unsafe { (*(user.as_ptr().cast::<TOKEN_USER>())).User.Sid };
    if !equal_sid(owner, user_sid) {
        return Err(io::Error::other("foreign-owned preferences"));
    }
    if dacl.is_null() {
        return Err(io::Error::other("NULL DACL permits other writers"));
    }
    let system = well_known_sid(WinLocalSystemSid)?;
    let administrators = well_known_sid(WinBuiltinAdministratorsSid)?;
    check_acl(
        dacl,
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
    let mut needed = 0;
    // SAFETY: this sizing call writes only the length; no buffer is supplied.
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
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
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &raw mut needed,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(buffer)
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
    if left.is_null() || right.is_null() {
        return false;
    }
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
        | FILE_APPEND_DATA
        | FILE_WRITE_EA
        | FILE_WRITE_ATTRIBUTES
        | FILE_DELETE_CHILD
        | DELETE
        | WRITE_DAC
        | WRITE_OWNER
        | GENERIC_WRITE
        | GENERIC_ALL;
    let sid = ptr::addr_of!(allowed.SidStart).cast_mut().cast();
    if allowed.Mask & write != 0 && !writers.iter().any(|writer| equal_sid(sid, *writer)) {
        return Err(io::Error::other("other-writable preferences"));
    }
    Ok(())
}
