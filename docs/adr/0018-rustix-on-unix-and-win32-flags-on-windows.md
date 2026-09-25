# The snapshot store uses rustix on Unix and Win32 flags on Windows

Status: accepted (owner decision, 2026-09-25).

maestro-core builds and passes its tests on Linux, macOS and Windows, the
first-class targets of [ADR-0016](0016-native-rust-desktop-workbench.md); CI's
portability legs run `cargo test` on `macos-15` and `windows-2025`. The
snapshot store and the tokenizer's artifact check reach the filesystem through
one internal interface, the crate's `filesystem` module, so neither branches on
the platform. On Linux and macOS it keeps rustix's `openat` family, a
`cfg(unix)` dependency. On Windows, which lacks `openat`, it uses `std::fs`
with the Win32 flags the standard library exposes safely through
`OpenOptionsExt` (`custom_flags`, `share_mode`) and `MetadataExt`
(`file_attributes`). The flags are Win32's documented values, named as
constants: no new dependency and no `unsafe` code.

The root the caller names, the CLI's `--output` directory or the path a
library caller passes, is trusted input: it resolves once through
`std::fs::canonicalize`, or through its deepest existing ancestor when saving
creates it. From that resolved root on, nothing may be a link. On loading, the
root is the directory above the snapshot's three identity directories. So
`/tmp/x` on macOS, where `/tmp` links into `/private`, behaves like `/tmp/x` on
Linux, and on Windows the resolved root is a verbatim path such as
`\\?\C:\data`. The threat the store answers is a link planted inside its tree,
by whoever can write there, to redirect a read or a write. The caller's own
path is not that tree, and every link below the resolved root stays refused.

Each guarantee the store documents holds on all three platforms:

| Guarantee | Linux and macOS | Windows |
| --- | --- | --- |
| Directory handles resist ancestor replacement | Each component opens relative to the handle before it | Every component, from the root on, is held open without `FILE_SHARE_DELETE` (with `FILE_FLAG_BACKUP_SEMANTICS`), so none can be renamed or removed while the store works under it |
| No link is followed, for directories or files | `O_NOFOLLOW` | `FILE_FLAG_OPEN_REPARSE_POINT` on every open, and any reparse point (`FILE_ATTRIBUTE_REPARSE_POINT`), junctions included, is refused |
| A FIFO is refused without blocking | `O_NONBLOCK`, then a file-type check | A directory holds no FIFO |
| Exclusive create of the pending file | `O_CREAT \| O_EXCL` | `create_new`, which is `CREATE_NEW` |
| Publish by hard link, so of two racing writers of different bytes one loses | `linkat` refuses an existing name | `fs::hard_link` refuses an existing name |
| Equal bytes reused, unequal existing files refused | The loser compares bytes | The same |
| `..` refused, and below the resolved root only names | The resolved root starts at `/` | The resolved root starts at its verbatim drive or share prefix, held open like every component |
| Files and directory entries synced | `fsync` on the file, then on the directory | `FlushFileBuffers` on the file only |

The last row differs. Windows flushes a directory only through a handle opened
for writing, so on Windows the store skips the directory flush. It still
flushes each file before linking it, so no partial artifact ever appears under
its final name. A power loss inside NTFS's journal-commit window can drop the
newest entry, and the next save recreates it. PostgreSQL and RocksDB make the
same choice.

## Considered options

- cap-std and cap-fs-ext 3.4.6, the Bytecode Alliance's capability-based
  filesystem API: on Windows they pull in winx, licensed
  `Apache-2.0 WITH LLVM-exception` alone, which the organization's licence
  list does not allow; widening it takes a chain of rust-workflows releases.
- windows-sys and `NtCreateFile` relative to a directory handle, as `openat`
  on Unix: its calls are `unsafe`, which the workspace forbids.
- `std::fs` by path, checked before each open, without holding directories:
  this brings back the check-then-open race that held handles close.
- Flush Windows directories through a handle opened for writing: only a CI
  leg could show whether NTFS accepts it, and the result would be the same.
- Refuse saving on Windows: this contradicts the platform goal.
- Refuse a link in every component of the caller's path, the root included:
  macOS then refuses `/tmp` and `/var`, links into `/private`, that Linux
  accepts, so one path behaves differently by platform, and no link inside
  the store tree is refused that resolving the root lets through.
