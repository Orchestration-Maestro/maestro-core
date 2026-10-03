//! Replaceable process containment and platform-specific kernel controls.
pub mod port;
// Linux kernel controls have no equivalent on the other supported hosts.
#[cfg(target_os = "linux")]
pub mod bootstrap;
#[cfg(target_os = "linux")]
mod cgroup;
#[cfg(target_os = "linux")]
mod cgroup_host;
#[cfg(target_os = "linux")]
mod elf;
#[cfg(target_os = "linux")]
mod launch;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
mod sandbox;
#[cfg(target_os = "linux")]
mod supervision;
#[cfg(target_os = "linux")]
mod syscalls;

#[cfg(target_os = "linux")]
mod scratch;
#[cfg(test)]
#[cfg(target_os = "linux")]
mod test_support;

#[cfg(test)]
#[cfg(target_os = "linux")]
mod child_tests;

#[cfg(all(test, target_os = "linux"))]
mod launch_metadata_tests;

#[cfg(all(test, target_os = "linux"))]
mod cgroup_decision_tests;

#[cfg(test)]
#[cfg(target_os = "linux")]
mod linux_configuration_tests;

#[cfg(test)]
#[cfg(target_os = "linux")]
mod guard_tests;

#[cfg(test)]
#[cfg(target_os = "linux")]
mod launch_flag_tests;

#[cfg(all(test, target_os = "linux"))]
mod bootstrap_command_tests;
#[cfg(target_os = "linux")]
mod bootstrap_host;
#[cfg(all(test, target_os = "linux"))]
mod bootstrap_tests;
#[cfg(test)]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod policy_child_tests;
#[cfg(target_os = "linux")]
mod sandbox_host;
#[cfg(all(test, target_os = "linux"))]
mod sandbox_tests;
#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod syscall_filter_tests;
