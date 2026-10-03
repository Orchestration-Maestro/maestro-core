//! Execute the compiled classic BPF against independent Linux seccomp input words.
use super::syscalls::compiled;
use nix::libc;
use seccompiler::BpfProgram;

/// Tiny interpreter for exactly the compiler's emitted instruction set, with bounded steps.
fn action(program: &BpfProgram, syscall: i64, args: [u64; 6]) -> u32 {
    let mut data = [0_u32; 16];
    data[0] = u32::try_from(syscall).unwrap();
    data[1] = 0xc000_003e;
    for (index, arg) in args.into_iter().enumerate() {
        *data.get_mut(4 + index * 2).unwrap() = u32::try_from(arg & 0xffff_ffff).unwrap();
        *data.get_mut(5 + index * 2).unwrap() = u32::try_from(arg >> 32).unwrap();
    }
    let mut accumulator = 0;
    let mut pc = 0;
    for _ in 0..program.len() * 2 {
        let instruction = program.get(pc).unwrap();
        pc += 1;
        match instruction.code {
            0x20 => {
                accumulator = *data
                    .get(usize::try_from(instruction.k / 4).unwrap())
                    .unwrap();
            }
            0x54 => accumulator &= instruction.k,
            0x05 => pc += usize::try_from(instruction.k).unwrap(),
            0x15 => {
                pc += usize::from(if accumulator == instruction.k {
                    instruction.jt
                } else {
                    instruction.jf
                });
            }
            0x06 => return instruction.k,
            code => panic!("unexpected BPF opcode {code:x}"),
        }
    }
    panic!("compiled filter did not terminate")
}
#[test]
fn n17_compiled_filters_clone3_enosys_default_eperm_and_allowed_calls() {
    let [fallback, profile] = compiled().unwrap();
    assert_eq!(action(&fallback, libc::SYS_clone3, [0; 6]), 0x0005_0026);
    for syscall in [
        libc::SYS_read,
        libc::SYS_write,
        libc::SYS_getpid,
        libc::SYS_openat,
        libc::SYS_clone,
        libc::SYS_execveat,
        libc::SYS_exit_group,
    ] {
        assert_eq!(action(&fallback, syscall, [0; 6]), 0x7fff_0000);
        assert_eq!(action(&profile, syscall, [0; 6]), 0x7fff_0000);
    }
    for syscall in [
        libc::SYS_socket,
        libc::SYS_unshare,
        libc::SYS_setns,
        libc::SYS_mount,
        libc::SYS_ptrace,
    ] {
        assert_eq!(action(&profile, syscall, [0; 6]), 0x0005_0001);
    }
}
#[test]
fn n17_compiled_filter_each_namespace_and_wx_mapping_denied() {
    let [_, profile] = compiled().unwrap();
    for flags in [
        libc::CLONE_NEWUSER,
        libc::CLONE_NEWNS,
        libc::CLONE_NEWPID,
        libc::CLONE_NEWNET,
        libc::CLONE_NEWIPC,
        libc::CLONE_NEWUTS,
        libc::CLONE_NEWCGROUP,
    ] {
        let mut args = [0; 6];
        args[0] = u64::try_from(flags).unwrap();
        assert_eq!(action(&profile, libc::SYS_clone, args), 0x0005_0001);
    }
    for (protection, flags, allowed) in [(3, 32, true), (5, 2, true), (7, 2, false), (5, 32, false)]
    {
        let mut args = [0; 6];
        args[2] = protection;
        args[3] = flags;
        assert_eq!(
            action(&profile, libc::SYS_mmap, args),
            if allowed { 0x7fff_0000 } else { 0x0005_0001 }
        );
    }
    for (protection, allowed) in [(3, true), (5, false), (7, false)] {
        let mut args = [0; 6];
        args[2] = protection;
        assert_eq!(
            action(&profile, libc::SYS_mprotect, args),
            if allowed { 0x7fff_0000 } else { 0x0005_0001 }
        );
    }
}
