//! The sole unsafe boundary: security information belongs to an already-held handle.
#![allow(
    unsafe_code,
    reason = "the sole audited held-handle Win32 security boundary"
)]
#![deny(clippy::undocumented_unsafe_blocks)]
use crate::windows_flags::{FILE_SHARE_READ_WRITE, OPEN_REPARSE_DIRECTORY_FLAGS};
use std::slice;
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
#[cfg(test)]
use windows_sys::Win32::{
    Foundation::{ERROR_NOT_ALL_ASSIGNED, ERROR_SUCCESS, GetLastError, LUID},
    Security::{
        AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TokenPrivileges,
    },
};
use windows_sys::Win32::{
    Foundation::{GENERIC_ALL, GENERIC_READ, GENERIC_WRITE, LocalFree},
    Security::{
        ACCESS_ALLOWED_ACE, ACE_HEADER, ACL,
        Authorization::{GetSecurityInfo, SE_FILE_OBJECT, SetSecurityInfo},
        CreateWellKnownSid, DACL_SECURITY_INFORMATION, EqualSid, GetAce,
        GetSecurityDescriptorControl, GetTokenInformation, INHERIT_ONLY_ACE, IsValidAcl,
        IsValidSid, OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSID,
        SE_DACL_PROTECTED, TOKEN_INFORMATION_CLASS, TOKEN_QUERY, TOKEN_USER, TokenUser,
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
        .read(true)
        .access_mode(DELETE | GENERIC_READ)
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

/// Memory allocated by `GetSecurityInfo`, which owns the returned SID and ACL pointers.
#[derive(Default)]
struct SecurityDescriptor {
    /// `LocalAlloc` allocation owning the returned security pointers.
    memory: *mut c_void,
    /// Owner SID within memory.
    owner: PSID,
    /// DACL within memory, possibly NULL.
    dacl: *mut ACL,
}
impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        // SAFETY: GetSecurityInfo allocated this descriptor with LocalAlloc; freed once here.
        unsafe {
            LocalFree(self.memory);
        }
    }
}

impl SecurityDescriptor {
    /// Protected versus inheritable DACL state, independent of ACL bytes.
    fn protection(&self) -> io::Result<u32> {
        let mut control = 0;
        let mut revision = 0;
        // SAFETY: GetSecurityInfo initialized the descriptor, retained throughout this call.
        if unsafe { GetSecurityDescriptorControl(self.memory, &raw mut control, &raw mut revision) }
            == 0
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

    /// Read owner and DACL from one live held file, freeing the allocation on drop.
    fn read(file: &File) -> io::Result<Self> {
        let mut security = Self::default();
        // SAFETY: live file handle and writable output slots; descriptor owns returned SID/ACL.
        let result = unsafe {
            GetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &raw mut security.owner,
                ptr::null_mut(),
                &raw mut security.dacl,
                ptr::null_mut(),
                &raw mut security.memory,
            )
        };
        if result != 0 {
            return Err(io::Error::from_raw_os_error(result.cast_signed()));
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
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION | protection,
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
    if !equal_sid(descriptor.owner, retained.owner)
        || descriptor.acl_bytes()? != retained.acl_bytes()?
        || protection != retained.protection()?
    {
        return Err(io::Error::other(
            "replacement would change the file's access protection; \
             its permissions or its folder's inheritable permissions differ; \
             fix them, then rerun",
        ));
    }
    Ok(())
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
fn token_information(
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
/// Temporarily disable ACL-bypass privileges; relies on nextest's one-process-per-test isolation.
#[cfg(test)]
pub(super) struct DisabledAclBypass {
    /// The process token, kept open until its previous privilege state is restored.
    token: OwnedHandle,
    /// Privilege identifiers used to verify the fixture's premise by reading the token back.
    luids: [LUID; 2],
    /// Previous states returned by Windows, including empty states for unchanged privileges.
    previous: [TOKEN_PRIVILEGES; 2],
}

#[cfg(test)]
impl DisabledAclBypass {
    /// Disable backup and restore privileges, restoring even a partially completed change on error.
    pub(super) fn new() -> io::Result<Self> {
        let mut handle = ptr::null_mut();
        // SAFETY: the pseudo-process handle is valid; output is a writable handle slot.
        if unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_QUERY | TOKEN_ADJUST_PRIVILEGES,
                &raw mut handle,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: successful OpenProcessToken transfers a unique handle closed by OwnedHandle.
        let token = unsafe { OwnedHandle::from_raw_handle(handle) };
        let mut guard = Self {
            token,
            luids: [LUID::default(); 2],
            previous: [TOKEN_PRIVILEGES::default(); 2],
        };
        for (index, name) in ["SeBackupPrivilege", "SeRestorePrivilege"]
            .into_iter()
            .enumerate()
        {
            let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
            // SAFETY: name is terminated and alive; the LUID output slot is writable.
            if unsafe {
                LookupPrivilegeValueW(ptr::null(), name.as_ptr(), &raw mut guard.luids[index])
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            let disabled = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES {
                    Luid: guard.luids[index],
                    Attributes: 0,
                }],
            };
            let mut needed = 0;
            // SAFETY: the token is live; both states hold one privilege and outputs are writable.
            let adjusted = unsafe {
                AdjustTokenPrivileges(
                    guard.token.as_raw_handle(),
                    0,
                    &raw const disabled,
                    u32::try_from(size_of::<TOKEN_PRIVILEGES>()).unwrap(),
                    &raw mut guard.previous[index],
                    &raw mut needed,
                )
            };
            // SAFETY: GetLastError has no pointer or lifetime requirements; read immediately.
            let error = unsafe { GetLastError() };
            if adjusted == 0 || (error != ERROR_SUCCESS && error != ERROR_NOT_ALL_ASSIGNED) {
                return Err(io::Error::from_raw_os_error(error.cast_signed()));
            }
            // A token without the privilege cannot bypass the ACL; readback verifies this too.
        }
        Ok(guard)
    }

    /// Read the current token back; absent privileges count as disabled, never as enabled.
    pub(super) fn both_disabled(&self) -> io::Result<bool> {
        let (buffer, needed) = token_information(&self.token, TokenPrivileges)?;
        let offset = std::mem::offset_of!(TOKEN_PRIVILEGES, Privileges);
        if (needed as usize) < offset {
            return Err(io::Error::other("invalid token privilege header"));
        }
        // SAFETY: the initialized buffer has at least the four-byte privilege-count header.
        let count = unsafe { *buffer.as_ptr().cast::<u32>() } as usize;
        if count > ((needed as usize) - offset) / size_of::<LUID_AND_ATTRIBUTES>() {
            return Err(io::Error::other("invalid token privilege count"));
        }
        // SAFETY: offset preserves alignment, count is bounded by initialized returned bytes,
        // and the owning buffer remains alive throughout the slice traversal.
        let privileges = unsafe {
            slice::from_raw_parts(
                buffer
                    .as_ptr()
                    .byte_add(offset)
                    .cast::<LUID_AND_ATTRIBUTES>(),
                count,
            )
        };
        Ok(!privileges.iter().any(|privilege| {
            self.luids.iter().any(|luid| {
                privilege.Luid.LowPart == luid.LowPart && privilege.Luid.HighPart == luid.HighPart
            }) && privilege.Attributes & SE_PRIVILEGE_ENABLED != 0
        }))
    }
}

#[cfg(test)]
impl Drop for DisabledAclBypass {
    fn drop(&mut self) {
        for previous in &self.previous {
            // SAFETY: the token is live and each previous state was returned by Windows;
            // no previous-state output buffer is requested during restoration.
            let restored = unsafe {
                AdjustTokenPrivileges(
                    self.token.as_raw_handle(),
                    0,
                    previous,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            };
            // SAFETY: GetLastError has no pointer or lifetime requirements; read immediately.
            let error = unsafe { GetLastError() };
            assert!(
                restored != 0 && error == ERROR_SUCCESS,
                "restoring token privileges: {error}"
            );
        }
    }
}
