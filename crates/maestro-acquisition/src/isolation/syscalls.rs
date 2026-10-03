//! Architecture-bound default-deny profile; namespace and management calls absent.
use super::port::Refusal;
#[cfg(target_arch = "x86_64")]
use nix::{libc, sched::CloneFlags};
#[cfg(target_arch = "x86_64")]
use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule, TargetArch,
};
#[cfg(target_arch = "x86_64")]
use std::collections::BTreeMap;

/// Descendants may clone normally, but cannot create any namespace.
#[cfg(target_arch = "x86_64")]
const NAMESPACES: CloneFlags = CloneFlags::CLONE_NEWUSER
    .union(CloneFlags::CLONE_NEWNS)
    .union(CloneFlags::CLONE_NEWPID)
    .union(CloneFlags::CLONE_NEWNET)
    .union(CloneFlags::CLONE_NEWIPC)
    .union(CloneFlags::CLONE_NEWUTS)
    .union(CloneFlags::CLONE_NEWCGROUP);

/// Clone permits descendants/threads, but never namespace creation or escape.
#[cfg(target_arch = "x86_64")]
fn clone_rule() -> Result<SeccompRule, Refusal> {
    SeccompRule::new(vec![
        SeccompCondition::new(
            0,
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::MaskedEq(
                u64::try_from(NAMESPACES.bits()).map_err(|_| Refusal::Containment)?,
            ),
            0,
        )
        .map_err(|_| Refusal::Containment)?,
    ])
    .map_err(|_| Refusal::Containment)
}
/// Require zero in these syscall flag bits.
#[cfg(target_arch = "x86_64")]
fn no_bits(argument: u8, mask: i32) -> Result<SeccompCondition, Refusal> {
    SeccompCondition::new(
        argument,
        SeccompCmpArgLen::Dword,
        SeccompCmpOp::MaskedEq(u64::try_from(mask).map_err(|_| Refusal::Containment)?),
        0,
    )
    .map_err(|_| Refusal::Containment)
}
/// Executable mappings must be file-backed RX, never generated anonymous code.
#[cfg(target_arch = "x86_64")]
fn mappings() -> Result<Vec<SeccompRule>, Refusal> {
    Ok(vec![
        SeccompRule::new(vec![no_bits(2, libc::PROT_EXEC)?]).map_err(|_| Refusal::Containment)?,
        SeccompRule::new(vec![
            no_bits(2, libc::PROT_WRITE)?,
            no_bits(3, libc::MAP_ANONYMOUS)?,
        ])
        .map_err(|_| Refusal::Containment)?,
    ])
}
/// x86-64 is the explicitly qualified Linux architecture, not a generic ABI guess.
#[cfg(target_arch = "x86_64")]
fn allowed() -> BTreeMap<i64, Vec<SeccompRule>> {
    let mut calls = BTreeMap::new();
    for call in [
        libc::SYS_read,
        libc::SYS_write,
        libc::SYS_readv,
        libc::SYS_writev,
        libc::SYS_pread64,
        libc::SYS_close,
        libc::SYS_close_range,
        libc::SYS_fstat,
        libc::SYS_newfstatat,
        libc::SYS_stat,
        libc::SYS_lstat,
        libc::SYS_statx,
        libc::SYS_lseek,
        libc::SYS_mmap,
        libc::SYS_mprotect,
        libc::SYS_munmap,
        libc::SYS_brk,
        libc::SYS_mremap,
        libc::SYS_madvise,
        libc::SYS_arch_prctl,
        libc::SYS_rt_sigaction,
        libc::SYS_rt_sigprocmask,
        libc::SYS_rt_sigreturn,
        libc::SYS_sigaltstack,
        libc::SYS_futex,
        libc::SYS_set_tid_address,
        libc::SYS_set_robust_list,
        libc::SYS_rseq,
        libc::SYS_getrandom,
        libc::SYS_clock_gettime,
        libc::SYS_clock_nanosleep,
        libc::SYS_nanosleep,
        libc::SYS_getpid,
        libc::SYS_getppid,
        libc::SYS_gettid,
        libc::SYS_getuid,
        libc::SYS_geteuid,
        libc::SYS_getgid,
        libc::SYS_getegid,
        libc::SYS_openat,
        libc::SYS_open,
        libc::SYS_access,
        libc::SYS_faccessat,
        libc::SYS_readlink,
        libc::SYS_readlinkat,
        libc::SYS_getdents64,
        libc::SYS_getcwd,
        libc::SYS_chdir,
        libc::SYS_fcntl,
        libc::SYS_dup,
        libc::SYS_dup2,
        libc::SYS_dup3,
        libc::SYS_pipe,
        libc::SYS_pipe2,
        libc::SYS_poll,
        libc::SYS_ppoll,
        libc::SYS_sched_yield,
        libc::SYS_sched_getaffinity,
        libc::SYS_prlimit64,
        libc::SYS_getrlimit,
        libc::SYS_uname,
        libc::SYS_mkdir,
        libc::SYS_mkdirat,
        libc::SYS_unlink,
        libc::SYS_unlinkat,
        libc::SYS_rename,
        libc::SYS_renameat,
        libc::SYS_fchmod,
        libc::SYS_chmod,
        libc::SYS_ftruncate,
        libc::SYS_fsync,
        libc::SYS_fdatasync,
        libc::SYS_execve,
        libc::SYS_execveat,
        libc::SYS_fork,
        libc::SYS_vfork,
        libc::SYS_wait4,
        libc::SYS_waitid,
        libc::SYS_kill,
        libc::SYS_tgkill,
        libc::SYS_exit,
        libc::SYS_exit_group,
        libc::SYS_clone3,
    ] {
        calls.insert(call, vec![]);
    }
    calls
}

/// Plain syscall plan also exercised without host setup in default tests.
#[cfg(target_arch = "x86_64")]
fn plan() -> Result<BTreeMap<i64, Vec<SeccompRule>>, Refusal> {
    let mut calls = allowed();
    calls.insert(libc::SYS_clone, vec![clone_rule()?]);
    calls.insert(libc::SYS_mmap, mappings()?);
    calls.insert(
        libc::SYS_mprotect,
        vec![
            SeccompRule::new(vec![no_bits(2, libc::PROT_EXEC)?])
                .map_err(|_| Refusal::Containment)?,
        ],
    );
    Ok(calls)
}
/// Compile both filters before applying either; default tests execute their actual BPF.
#[cfg(target_arch = "x86_64")]
pub(super) fn compiled() -> Result<[BpfProgram; 2], Refusal> {
    let calls = plan()?;
    // glibc falls back to clone only on ENOSYS; clone3's pointed flags cannot be filtered.
    let fallback: BpfProgram = SeccompFilter::new(
        BTreeMap::from([(libc::SYS_clone3, vec![])]),
        SeccompAction::Allow,
        SeccompAction::Errno(38),
        TargetArch::x86_64,
    )
    .map_err(|_| Refusal::Containment)?
    .try_into()
    .map_err(|_| Refusal::Containment)?;
    let profile: BpfProgram = SeccompFilter::new(
        calls,
        SeccompAction::Errno(1),
        SeccompAction::Allow,
        TargetArch::x86_64,
    )
    .map_err(|_| Refusal::Containment)?
    .try_into()
    .map_err(|_| Refusal::Containment)?;
    Ok([fallback, profile])
}
/// Apply the qualified architecture's filters; other Linux architectures refuse.
pub(super) fn restrict() -> Result<(), Refusal> {
    #[cfg(target_arch = "x86_64")]
    {
        for filter in &compiled()? {
            seccompiler::apply_filter(filter).map_err(|_| Refusal::Unsupported)?;
        }
        Ok(())
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        Err(Refusal::Unsupported)
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::{libc, plan};
    use seccompiler::{SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompRule};

    /// Independent Linux UAPI condition, not the policy builder under test.
    #[expect(
        clippy::unwrap_used,
        reason = "independently authored valid UAPI test conditions"
    )]
    fn zero_bits(argument: u8, mask: u64) -> SeccompCondition {
        SeccompCondition::new(
            argument,
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::MaskedEq(mask),
            0,
        )
        .unwrap()
    }
    #[test]
    fn n17_default_syscall_plan_denies_bypasses_and_exec_memory_flags() {
        let plan = plan().unwrap();
        for syscall in [
            libc::SYS_socket,
            libc::SYS_connect,
            libc::SYS_unshare,
            libc::SYS_setns,
            libc::SYS_mount,
            libc::SYS_ptrace,
            libc::SYS_process_vm_writev,
            libc::SYS_personality,
            libc::SYS_memfd_create,
            libc::SYS_shmget,
            libc::SYS_shmat,
            libc::SYS_io_uring_setup,
            libc::SYS_io_uring_enter,
            libc::SYS_io_uring_register,
            libc::SYS_userfaultfd,
            libc::SYS_pkey_mprotect,
        ] {
            assert!(!plan.contains_key(&syscall));
        }
        assert_eq!(
            plan.get(&libc::SYS_mmap),
            Some(&vec![
                SeccompRule::new(vec![zero_bits(2, 4)]).unwrap(),
                SeccompRule::new(vec![zero_bits(2, 2), zero_bits(3, 32)]).unwrap(),
            ])
        );
        assert_eq!(
            plan.get(&libc::SYS_mprotect),
            Some(&vec![SeccompRule::new(vec![zero_bits(2, 4)]).unwrap()])
        );
        assert_eq!(
            plan.get(&libc::SYS_clone),
            Some(&vec![
                SeccompRule::new(vec![zero_bits(0, 0x7e02_0000)]).unwrap()
            ])
        );
        assert_eq!(plan.get(&libc::SYS_read), Some(&vec![]));
        assert_eq!(plan.get(&libc::SYS_write), Some(&vec![]));
    }
}
