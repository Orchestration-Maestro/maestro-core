//! Kernel mount/capability effects only, using the driver's prepared values.
use super::port::{Mount, Refusal, SandboxIo};
use nix::{
    mount::{MntFlags, mount, umount2},
    unistd::{chdir, pivot_root},
};
use rustix::thread::{
    CapabilitiesSecureBits, CapabilitySets, clear_ambient_capability_set, set_capabilities,
    set_capabilities_secure_bits,
};
use std::{fs, io, path::Path};

/// Mandatory kernel implementation; no configurable plain-process fallback.
pub(super) struct Host;
impl SandboxIo for Host {
    fn mount(&self, operation: Mount<'_>) -> Result<(), Refusal> {
        mount(
            operation.source,
            operation.target,
            operation.filesystem,
            operation.flags,
            operation.options,
        )
        .map_err(|_| Refusal::Unsupported)
    }
    fn pivot(&self, root: &Path, old: &Path) -> Result<(), Refusal> {
        pivot_root(root, old).map_err(|_| Refusal::Unsupported)
    }
    fn chdir(&self, path: &Path) -> Result<(), Refusal> {
        chdir(path).map_err(|_| Refusal::Containment)
    }
    fn unmount(&self, path: &Path, flags: MntFlags) -> Result<(), Refusal> {
        umount2(path, flags).map_err(|_| Refusal::Containment)
    }
    fn securebits(&self, bits: CapabilitiesSecureBits) -> Result<(), Refusal> {
        set_capabilities_secure_bits(bits).map_err(|_| Refusal::Unsupported)
    }
    fn clear_ambient(&self) -> Result<(), Refusal> {
        clear_ambient_capability_set().map_err(|_| Refusal::Unsupported)
    }
    fn capabilities(&self, sets: CapabilitySets) -> Result<(), Refusal> {
        set_capabilities(None, sets).map_err(|_| Refusal::Unsupported)
    }
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(path)
    }
}
