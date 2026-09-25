# The snapshot store reaches the filesystem through cap-std

Status: accepted (owner decision, 2026-09-25).

maestro-core builds and passes its tests on Linux, macOS and Windows, the
first-class targets of [ADR-0016](0016-native-rust-desktop-workbench.md); CI's
portability legs run `cargo test` on `macos-15` and `windows-2025`. The
snapshot store and the tokenizer's artifact check used rustix's `openat`
family, which Windows lacks. They now use `cap-std` and `cap-fs-ext` 3.4.6,
the Bytecode Alliance's capability-based filesystem API. A `Dir` is a
directory handle every lookup starts from. It works through rustix on Linux
and macOS and through the native API on Windows, and it needs no `unsafe` code
from us.

Each guarantee the store documents holds on all three platforms:

| Guarantee | Linux and macOS | Windows |
| --- | --- | --- |
| Directory handles resist ancestor replacement | Each component opens relative to the handle before it | cap-std opens directories without `FILE_SHARE_DELETE`, so an open directory cannot be renamed or removed, and each operation starts from the handle's current path |
| No link is followed, for directories or files | `O_NOFOLLOW` | `FILE_FLAG_OPEN_REPARSE_POINT`, and any reparse point, junctions included, is refused |
| A FIFO is refused without blocking | `O_NONBLOCK`, then a file-type check | A directory holds no FIFO |
| Exclusive create of the pending file | `O_CREAT \| O_EXCL` | `CREATE_NEW` |
| Publish by hard link, so of two racing writers of different bytes one loses | `linkat` refuses an existing name | `CreateHardLinkW` refuses an existing name |
| Equal bytes reused, unequal existing files refused | The loser compares bytes | The same |
| Files and directory entries synced | `fsync` on the file, then on the directory | `FlushFileBuffers` on the file only |

Only the last row differs. Windows flushes a directory only through a writable
handle, and cap-std opens directories read-only, so on Windows the store skips
the directory flush. It still flushes each file before linking it, so no
partial artifact ever appears under its final name. A power loss inside NTFS's
journal-commit window can drop the newest entry, and the next save recreates
it. PostgreSQL and RocksDB make the same choice.

A path is walked from its root (on Windows, its drive or share prefix) or from
the working directory; `..` is still refused. On macOS, `/tmp` and `/var` are
links into `/private`, which the store refuses like any other link: give it a
link-free path such as `/private/tmp`.

The pin is 3.4.6, not 4.x. cap-std 4.0.3 pulls in windows-sys 0.59, 0.60 and
0.61 and io-lifetimes 2 and 3, through fs-set-times, winx and io-extras, and
DEP-001 allows one version of each crate. 3.4.6, with windows-sys locked at
0.59, has one version of each. A bump to 4.x waits until its dependencies
agree on one windows-sys.

The new crates are licensed `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR
MIT` or `MIT OR Apache-2.0`, except winx, cap-std's Windows dependency, which
is `Apache-2.0 WITH LLVM-exception` alone. The LLVM exception only widens
Apache-2.0, which the organization already allows, so the organization's
reviewed licences were widened to include it, in a rust-workflows release to
come.

## Considered options

- Keep rustix and write the Windows calls by hand: windows-sys calls are
  `unsafe`, which the workspace forbids.
- `std::fs` by path, checked before each open: this brings back the
  check-then-open race that the handles close.
- Flush Windows directories through a handle opened for writing: only a CI
  leg could show whether NTFS accepts it, and the result would be the same.
- Refuse saving on Windows: this contradicts the platform goal.
