//! Minimal private filesystem, capability removal and strict Landlock V3.
use super::port::{Mount, Refusal, SandboxIo};
use landlock::{
    ABI, Access as _, AccessFs, BitFlags, CompatLevel, Compatible as _, PathBeneath, PathFd,
    Ruleset, RulesetAttr as _, RulesetCreatedAttr as _, RulesetStatus,
};
use nix::mount::{MntFlags, MsFlags};
use rustix::thread::{CapabilitiesSecureBits, CapabilitySet, CapabilitySets, set_no_new_privs};
use std::path::Path;

/// Pivoting removes proc roots and management socket paths; policy stays outside the leaf.
pub(super) fn filesystem_with(
    root: &Path,
    memory_bytes: u64,
    effects: &impl SandboxIo,
) -> Result<(), Refusal> {
    let [private, bind, input_readonly, scratch_noexec, root_readonly] = mount_plan();
    effects.mount(Mount {
        source: None,
        target: Path::new("/"),
        filesystem: None,
        flags: private,
        options: None,
    })?;
    effects.mount(Mount {
        source: Some(root),
        target: root,
        filesystem: None,
        flags: bind,
        options: None,
    })?;
    let input = root.join("input");
    effects.mount(Mount {
        source: Some(&input),
        target: &input,
        filesystem: None,
        flags: bind,
        options: None,
    })?;
    effects.mount(Mount {
        source: None,
        target: &input,
        filesystem: None,
        flags: input_readonly,
        options: None,
    })?;
    let scratch = root.join("work");
    let options = format!("size={memory_bytes},mode=0700");
    effects.mount(Mount {
        source: Some(Path::new("tmpfs")),
        target: &scratch,
        filesystem: Some("tmpfs"),
        flags: scratch_noexec,
        options: Some(&options),
    })?;
    effects.pivot(root, &root.join("old-root"))?;
    effects.chdir(Path::new("/"))?;
    effects.unmount(Path::new("/old-root"), MntFlags::MNT_DETACH)?;
    effects.mount(Mount {
        source: None,
        target: Path::new("/"),
        filesystem: None,
        flags: root_readonly,
        options: None,
    })?;
    effects.chdir(Path::new("/work"))
}
/// Fully enforced V3 is a hard requirement, never the library's best effort.
pub(super) fn landlock(root: &Path, interpreter: Option<&str>) -> Result<(), Refusal> {
    let policy = file_plan();
    let mut rules = Ruleset::default()
        .set_compatibility(policy.compatibility)
        .handle_access(policy.handled)
        .map_err(|_| Refusal::Unsupported)?
        .create()
        .map_err(|_| Refusal::Unsupported)?
        .add_rule(PathBeneath::new(
            PathFd::new(root).map_err(|_| Refusal::Containment)?,
            policy.read,
        ))
        .map_err(|_| Refusal::Unsupported)?
        .add_rule(PathBeneath::new(
            PathFd::new(root.join("work")).map_err(|_| Refusal::Containment)?,
            policy.scratch,
        ))
        .map_err(|_| Refusal::Unsupported)?
        .add_rule(PathBeneath::new(
            PathFd::new(root.join("parser")).map_err(|_| Refusal::Containment)?,
            policy.executable,
        ))
        .map_err(|_| Refusal::Unsupported)?;
    if let Some(loader) = interpreter {
        rules = rules
            .add_rule(PathBeneath::new(
                PathFd::new(root.join(loader)).map_err(|_| Refusal::Containment)?,
                policy.executable,
            ))
            .map_err(|_| Refusal::Unsupported)?;
    }
    set_no_new_privs(true).map_err(|_| Refusal::Unsupported)?;
    let status = rules.restrict_self().map_err(|_| Refusal::Unsupported)?;
    enforced(&status.ruleset)
}
/// Only hard, fully enforced Landlock can qualify.
pub(super) fn enforced(status: &RulesetStatus) -> Result<(), Refusal> {
    if *status != file_plan().status {
        return Err(Refusal::Unsupported);
    }
    Ok(())
}
/// The security policy and visibility refusal remain ordinary tested decisions.
pub(super) fn capabilities_with(effects: &impl SandboxIo) -> Result<(), Refusal> {
    effects.securebits(SECUREBITS)?;
    effects.clear_ambient()?;
    effects.capabilities(CapabilitySets {
        effective: CapabilitySet::empty(),
        permitted: CapabilitySet::empty(),
        inheritable: CapabilitySet::empty(),
    })?;
    if effects
        .read(Path::new("/sys/fs/cgroup/cgroup.procs"))
        .is_ok()
    {
        return Err(Refusal::Containment);
    }
    Ok(())
}
/// Lock out root regain independently of the empty capability sets.
const SECUREBITS: CapabilitiesSecureBits =
    CapabilitiesSecureBits::NO_ROOT.union(CapabilitiesSecureBits::NO_ROOT_LOCKED);
/// Recursively private propagation prevents sharing future host mounts.
const PRIVATE: MsFlags = MsFlags::MS_REC.union(MsFlags::MS_PRIVATE);
/// Input is immutable, nonexecutable and cannot introduce devices or privilege.
const INPUT: MsFlags = MsFlags::MS_BIND
    .union(MsFlags::MS_REMOUNT)
    .union(MsFlags::MS_RDONLY)
    .union(MsFlags::MS_NOEXEC)
    .union(MsFlags::MS_NOSUID)
    .union(MsFlags::MS_NODEV);
/// Scratch never supplies executable or privileged mappings/devices.
const SCRATCH: MsFlags = MsFlags::MS_NOEXEC
    .union(MsFlags::MS_NOSUID)
    .union(MsFlags::MS_NODEV);
/// The full root is immutable after pivot; pinned executable paths retain execution.
const ROOT: MsFlags = MsFlags::MS_BIND
    .union(MsFlags::MS_REMOUNT)
    .union(MsFlags::MS_RDONLY)
    .union(MsFlags::MS_NOSUID)
    .union(MsFlags::MS_NODEV);
/// Concrete mount modes shared by the driver and default tests.
fn mount_plan() -> [MsFlags; 5] {
    [PRIVATE, MsFlags::MS_BIND, INPUT, SCRATCH, ROOT]
}
/// Root access deliberately omits Execute. enumflags2's const union is `union_c`.
const READ: BitFlags<AccessFs> =
    BitFlags::<AccessFs>::from_bits_truncate_c(AccessFs::ReadFile as u64, BitFlags::CONST_TOKEN)
        .union_c(BitFlags::<AccessFs>::from_bits_truncate_c(
            AccessFs::ReadDir as u64,
            BitFlags::CONST_TOKEN,
        ));
/// Strict filesystem policy as data, not the crate's best-effort defaults.
struct FilePlan {
    /// Minimum handled filesystem ABI rights, including Refer and Truncate.
    handled: BitFlags<AccessFs>,
    /// Only fully compatible enforcement is acceptable.
    compatibility: CompatLevel,
    /// Immutable root read access, with no ambient Execute right.
    read: BitFlags<AccessFs>,
    /// Private scratch writes, never executable.
    scratch: BitFlags<AccessFs>,
    /// Only pinned image paths receive Execute.
    executable: BitFlags<AccessFs>,
    /// Required actual kernel result, not a best-effort status.
    status: RulesetStatus,
}
/// One source of truth for the filesystem profile's security posture.
fn file_plan() -> FilePlan {
    FilePlan {
        handled: AccessFs::from_all(ABI::V3),
        compatibility: CompatLevel::HardRequirement,
        read: READ,
        scratch: AccessFs::from_all(ABI::V3) & !AccessFs::Execute,
        executable: AccessFs::Execute.into(),
        status: RulesetStatus::FullyEnforced,
    }
}
#[cfg(test)]
mod tests {
    use super::{CompatLevel, RulesetStatus, file_plan, mount_plan};
    #[test]
    fn n17_default_filesystem_plan_is_strict_v3_readonly_and_noexec() {
        let mounts = mount_plan().map(|flags| flags.bits());
        assert_eq!(mounts, [0x0004_4000, 0x1000, 0x102f, 0x000e, 0x1027]);
        let files = file_plan();
        assert_eq!(files.handled.bits(), 0x7fff);
        assert_eq!(files.compatibility, CompatLevel::HardRequirement);
        assert_eq!(files.read.bits(), 0x000c);
        assert_eq!(files.scratch.bits(), 0x7ffe);
        assert_eq!(files.executable.bits(), 1);
        assert_eq!(files.status, RulesetStatus::FullyEnforced);
    }
}
