//! Minimal private filesystem, capability removal and strict Landlock V3.
use super::{launch::Configuration, port::Refusal};
use landlock::{
    ABI, Access as _, AccessFs, BitFlags, CompatLevel, Compatible as _, PathBeneath, PathFd,
    Ruleset, RulesetAttr as _, RulesetCreatedAttr as _, RulesetStatus,
};
use nix::{
    mount::{MntFlags, MsFlags, mount, umount2},
    unistd::{chdir, pivot_root},
};
use rustix::thread::{
    CapabilitiesSecureBits, CapabilitySet, CapabilitySets, clear_ambient_capability_set,
    set_capabilities, set_capabilities_secure_bits, set_no_new_privs,
};
use std::{fs, path::Path};

/// Pivoting removes even `/proc/<host-pid>/root` and management socket paths.
pub(super) fn filesystem(config: &Configuration) -> Result<(), Refusal> {
    let [private, bind, input_readonly, scratch_noexec, root_readonly] = mount_plan();
    mount(None::<&str>, "/", None::<&str>, private, None::<&str>)
        .map_err(|_| Refusal::Unsupported)?;
    mount(
        Some(&config.root),
        &config.root,
        None::<&str>,
        bind,
        None::<&str>,
    )
    .map_err(|_| Refusal::Unsupported)?;
    let input = config.root.join("input");
    mount(Some(&input), &input, None::<&str>, bind, None::<&str>)
        .map_err(|_| Refusal::Unsupported)?;
    mount(
        None::<&str>,
        &input,
        None::<&str>,
        input_readonly,
        None::<&str>,
    )
    .map_err(|_| Refusal::Unsupported)?;
    let scratch = config.root.join("work");
    let options = format!("size={},mode=0700", config.memory_bytes);
    mount(
        Some("tmpfs"),
        &scratch,
        Some("tmpfs"),
        scratch_noexec,
        Some(options.as_str()),
    )
    .map_err(|_| Refusal::Unsupported)?;
    pivot_root(&config.root, &config.root.join("old-root")).map_err(|_| Refusal::Unsupported)?;
    chdir("/").map_err(|_| Refusal::Containment)?;
    umount2("/old-root", MntFlags::MNT_DETACH).map_err(|_| Refusal::Containment)?;
    mount(None::<&str>, "/", None::<&str>, root_readonly, None::<&str>)
        .map_err(|_| Refusal::Unsupported)?;
    chdir("/work").map_err(|_| Refusal::Containment)?;
    Ok(())
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
    if status.ruleset != policy.status {
        return Err(Refusal::Unsupported);
    }
    Ok(())
}
/// Clear ambient/permitted/effective/inheritable caps and lock out UID-zero regain.
pub(super) fn capabilities() -> Result<(), Refusal> {
    set_capabilities_secure_bits(
        CapabilitiesSecureBits::NO_ROOT | CapabilitiesSecureBits::NO_ROOT_LOCKED,
    )
    .map_err(|_| Refusal::Unsupported)?;
    clear_ambient_capability_set().map_err(|_| Refusal::Unsupported)?;
    set_capabilities(
        None,
        CapabilitySets {
            effective: CapabilitySet::empty(),
            permitted: CapabilitySet::empty(),
            inheritable: CapabilitySet::empty(),
        },
    )
    .map_err(|_| Refusal::Unsupported)?;
    // The root is read-only and scratch/input cannot supply executable mappings.
    if fs::read("/sys/fs/cgroup/cgroup.procs").is_ok() {
        return Err(Refusal::Containment);
    }
    Ok(())
}

/// Concrete mount flags, shared by the syscall adapter and default pure tests.
fn mount_plan() -> [MsFlags; 5] {
    [
        MsFlags::MS_REC | MsFlags::MS_PRIVATE,
        MsFlags::MS_BIND,
        MsFlags::MS_BIND
            | MsFlags::MS_REMOUNT
            | MsFlags::MS_RDONLY
            | MsFlags::MS_NOEXEC
            | MsFlags::MS_NOSUID
            | MsFlags::MS_NODEV,
        MsFlags::MS_NOEXEC | MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
        MsFlags::MS_BIND
            | MsFlags::MS_REMOUNT
            | MsFlags::MS_RDONLY
            | MsFlags::MS_NOSUID
            | MsFlags::MS_NODEV,
    ]
}
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
        read: AccessFs::ReadFile | AccessFs::ReadDir,
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
