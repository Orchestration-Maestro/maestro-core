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
