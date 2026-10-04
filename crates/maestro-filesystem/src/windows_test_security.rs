//! Test-only native token privileges and ACL fixtures; production stays in `windows_security`.
#![allow(
    unsafe_code,
    reason = "native test-only process token privilege fixtures"
)]
#![deny(clippy::undocumented_unsafe_blocks)]
use crate::windows_security::token_information;
use std::{
    ffi::c_void,
    io,
    os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle},
    ptr, slice,
};
use windows_sys::Win32::{
    Foundation::{ERROR_NOT_ALL_ASSIGNED, ERROR_SUCCESS, GENERIC_ALL, GetLastError, LUID},
    Security::{
        ACCESS_ALLOWED_ACE, ACL, AddAccessAllowedAce, AdjustTokenPrivileges, INHERIT_ONLY_ACE,
        InitializeAcl, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, PSID, SE_PRIVILEGE_ENABLED,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY, TokenPrivileges,
    },
    Storage::FileSystem::FILE_READ_DATA,
    System::{
        SystemServices::{ACCESS_ALLOWED_ACE_TYPE, ACCESS_DENIED_ACE_TYPE},
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
};
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

/// Construct one native allow ACL; its storage and the caller's SID live throughout the probe.
#[cfg(test)]
pub(super) fn with_allow_acl(sid: PSID, mask: u32, run: impl FnOnce(*mut ACL)) {
    let mut buffer = [0u32; 32];
    let acl = buffer.as_mut_ptr().cast::<ACL>();
    let bytes = u32::try_from(size_of_val(&buffer)).unwrap();
    // SAFETY: aligned writable ACL storage and caller-owned initialized SID remain alive.
    unsafe {
        assert_ne!(InitializeAcl(acl, bytes, 2), 0);
        assert_ne!(AddAccessAllowedAce(acl, 2, mask, sid), 0);
    }
    run(acl);
}

/// Exercise exact ACE size/count boundaries in aligned initialized native ACL storage.
#[cfg(test)]
pub(super) fn probe_acl_shapes(
    sid: PSID,
    mut observe_acl: impl FnMut(*mut ACL, bool),
    mut observe_ace: impl FnMut(*mut c_void, &str, bool),
) {
    let mut buffer = [0u32; 32];
    let acl = buffer.as_mut_ptr().cast::<ACL>();
    let bytes = u32::try_from(size_of_val(&buffer)).unwrap();
    // SAFETY: aligned writable ACL storage and caller-owned initialized SID remain alive.
    unsafe {
        assert_ne!(InitializeAcl(acl, bytes, 2), 0);
        assert_ne!(AddAccessAllowedAce(acl, 2, GENERIC_ALL, sid), 0);
    }
    // SAFETY: the native ACL header is followed by its initialized standard allow ACE.
    let ace = unsafe {
        buffer
            .as_mut_ptr()
            .byte_add(size_of::<ACL>())
            .cast::<ACCESS_ALLOWED_ACE>()
    };
    // SAFETY: ace stays inside the live aligned buffer throughout all field modifications.
    unsafe {
        (*ace).Mask = FILE_READ_DATA;
    }
    observe_ace(ace.cast(), "", false);
    // SAFETY: change only initialized mask and header fields of the live standard ACE.
    unsafe {
        (*ace).Header.AceFlags = u8::try_from(INHERIT_ONLY_ACE).unwrap();
        (*ace).Mask = GENERIC_ALL;
    }
    observe_acl(acl, true);
    // SAFETY: change the live ACE's header to unsupported, then standard denied type.
    unsafe {
        (*ace).Header.AceFlags = 0;
        (*ace).Header.AceType = 0xff;
    }
    observe_acl(acl, false);
    // SAFETY: standard denied ACE uses the same initialized mask/SID layout as allow ACE.
    unsafe {
        (*ace).Header.AceType = u8::try_from(ACCESS_DENIED_ACE_TYPE).unwrap();
    }
    observe_acl(acl, true);
    // SAFETY: backing storage retains every inspected byte, even for invalid declared sizes.
    unsafe {
        (*ace).Header.AceType = u8::try_from(ACCESS_ALLOWED_ACE_TYPE).unwrap();
        (*ace).Header.AceSize = 15;
    }
    observe_ace(ace.cast(), "ACE size", false);
    // SAFETY: count/size mismatch is deliberate; initialized bytes still fit the backing buffer.
    unsafe {
        (*ace).Header.AceSize = 16;
        *ace.cast::<u8>().add(9) = 1;
    }
    observe_ace(ace.cast(), "ACE SID", false);
    // SAFETY: the minimum ACE contains the zero-subauthority SID header, with read-only mask.
    unsafe {
        *ace.cast::<u8>().add(9) = 0;
        (*ace).Mask = FILE_READ_DATA;
    }
    observe_ace(ace.cast(), "", false);
    // SAFETY: restore the full initialized native SID/ACE and trust its writer explicitly.
    unsafe {
        (*ace).Header.AceSize = 20;
        *ace.cast::<u8>().add(9) = 1;
        (*ace).Mask = GENERIC_ALL;
    }
    observe_ace(ace.cast(), "", true);
    // SAFETY: invalid revision is within the live ACL header; refused before ACE traversal.
    unsafe {
        (*acl).AclRevision = 0;
    }
    observe_acl(acl, false);
}
