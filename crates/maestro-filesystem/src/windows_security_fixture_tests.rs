//! Safe native commands for independent Windows security observations.
#![cfg(all(test, windows))]
use std::{path::Path, process::Command};

#[cfg(test)]
pub(super) fn icacls(path: &Path, arguments: &[&str]) {
    assert!(
        Command::new("icacls")
            .arg(path)
            .args(arguments)
            .status()
            .unwrap()
            .success()
    );
}

#[cfg(test)]
pub(super) fn native_dacl(path: &Path) -> Vec<u8> {
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            concat!(
                "$d=[System.Security.AccessControl.RawSecurityDescriptor]::new(",
                "(Get-Acl -LiteralPath $env:MAESTRO_SECURITY_FIXTURE).",
                "GetSecurityDescriptorBinaryForm(),0); ",
                "$b=[byte[]]::new($d.DiscretionaryAcl.BinaryLength); ",
                "$d.DiscretionaryAcl.GetBinaryForm($b,0); [BitConverter]::ToString($b)",
            ),
        ])
        .env("MAESTRO_SECURITY_FIXTURE", path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .split('-')
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}

#[cfg(test)]
pub(super) fn native_owner(path: &Path) -> String {
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            concat!(
                "(Get-Acl -LiteralPath $env:MAESTRO_SECURITY_FIXTURE).",
                "GetOwner([System.Security.Principal.SecurityIdentifier]).Value",
            ),
        ])
        .env("MAESTRO_SECURITY_FIXTURE", path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
