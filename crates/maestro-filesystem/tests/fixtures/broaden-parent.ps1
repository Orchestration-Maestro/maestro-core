param([string]$Root, [switch]$SnapshotOnly)
$ErrorActionPreference = 'Stop'
$leafPath = Join-Path $Root 'target'
$leaf = Get-Acl -LiteralPath $leafPath
$original = [Convert]::ToBase64String($leaf.GetSecurityDescriptorBinaryForm())
function Snapshot($acl) {
$descriptor = [System.Security.AccessControl.RawSecurityDescriptor]::new(
    $acl.GetSecurityDescriptorBinaryForm(), 0)
$bytes = [byte[]]::new($descriptor.DiscretionaryAcl.BinaryLength)
$descriptor.DiscretionaryAcl.GetBinaryForm($bytes, 0)
    return '{0}|{1}|{2}' -f $descriptor.Owner.Value,
    [Convert]::ToBase64String($bytes), $acl.AreAccessRulesProtected
 }
$snapshot = Snapshot $leaf
if ($SnapshotOnly) { $snapshot; exit }
if ($leaf.AreAccessRulesProtected) { throw 'fixture leaf must be unprotected' }
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class NativeAcl {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateFile(string path, uint access, uint share, IntPtr security,
        uint creation, uint flags, IntPtr template);
    [DllImport("advapi32.dll", SetLastError = true)]
    public static extern bool SetKernelObjectSecurity(IntPtr handle, uint information, byte[] descriptor);
    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool CloseHandle(IntPtr handle);
}
'@
$acl = Get-Acl -LiteralPath $Root
$sid = [System.Security.Principal.SecurityIdentifier]::new('S-1-1-0')
$rule = [System.Security.AccessControl.FileSystemAccessRule]::new(
    $sid, 'ReadAndExecute', 'ContainerInherit,ObjectInherit', 'None', 'Allow')
$acl.AddAccessRule($rule)
# SetKernelObjectSecurity performs no inheritance propagation to existing children.
$handle = [NativeAcl]::CreateFile($Root, 0x40000, 3, [IntPtr]::Zero,
    3, 0x02200000, [IntPtr]::Zero)
if ($handle -eq [IntPtr]::new(-1)) {
    throw [System.ComponentModel.Win32Exception]::new(
        [Runtime.InteropServices.Marshal]::GetLastWin32Error())
}
try {
    if (-not [NativeAcl]::SetKernelObjectSecurity($handle, 4, $acl.GetSecurityDescriptorBinaryForm())) {
        throw [System.ComponentModel.Win32Exception]::new(
            [Runtime.InteropServices.Marshal]::GetLastWin32Error())
    }
} finally {
    if (-not [NativeAcl]::CloseHandle($handle)) { throw 'closing parent handle failed' }
}
# NTFS can protect/reorder an existing child while the parent DACL is changed.
# Restore the exact original unprotected DACL before testing the replacement.
$restore = [NativeAcl]::CreateFile($leafPath, 0x40000, 3, [IntPtr]::Zero, 3, 0x00200000, [IntPtr]::Zero)
if ($restore -eq [IntPtr]::new(-1)) {
    throw [System.ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error())
}
try {
    if (-not [NativeAcl]::SetKernelObjectSecurity($restore, 0x20000004, [Convert]::FromBase64String($original))) {
        throw [System.ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error())
    }
} finally { if (-not [NativeAcl]::CloseHandle($restore)) { throw 'closing leaf handle failed' } }
$sections = [System.Security.AccessControl.AccessControlSections]::Access
$parent = Get-Acl -LiteralPath $Root
if ($parent.GetSecurityDescriptorSddlForm($sections) -eq
    $leaf.GetSecurityDescriptorSddlForm($sections)) {
    throw 'parent DACL must differ from the leaf DACL'
}
if ((Snapshot (Get-Acl -LiteralPath $leafPath)) -ne $snapshot) {
    throw 'broadening the parent must not change the leaf security'
}
$snapshot
