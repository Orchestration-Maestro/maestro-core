# Research: qualifying lbug (G25)

G25 qualified [`lbug`](https://crates.io/crates/lbug), the Rust crate of the
embedded LadybugDB graph engine, against the six-row adoption bar of
[plan.md](plan.md) A1. This file holds the verdict, the measurements behind it
and the rulings that followed. ADR-0021 records the resulting design.

## Combined-pin qualification (802abe2)

On 2026-10-02, the opt-in spike qualified the combined E01/E02/E03/E03b
fork at `802abe2ab0fb55ceb72531caf9d0cae059670c4b`. `Cargo.toml` and
`Cargo.lock` name that identical immutable revision. This moves the pin
before E07a; it does not wire or qualify the product adapter, complete G25's
remaining cost measurements, or close the historical row-3 verdict below.

The hosted command was `cargo test -p lbug-spike --features engine --locked
--timings`. Build times are Cargo's complete test-profile build (including
native source compilation and Rust), not isolated C++ timings. Job times
include checkout, toolchain setup, tests and native-link inspection.

| OS | Build time / job time | Tests passed | Run URL | No-OpenSSL result |
| --- | --- | --- | --- | --- |
| Linux (`ubuntu-latest`) | 17:59 / 18:12 | 6; 0 failed | [three-OS run, Linux job](https://github.com/Orchestration-Maestro/maestro-core/actions/runs/36978566307/job/110747691004) | `ldd`: no `libssl` or `libcrypto`; loaded-library and disabled-installer tests pass |
| macOS (`macos-latest`) | 11:29 / 11:55; link-proof rerun 15:11 / 15:45 | 5; 0 failed in each run | [three-OS run, macOS job](https://github.com/Orchestration-Maestro/maestro-core/actions/runs/36978566307/job/110747691024); [executable-link proof](https://github.com/Orchestration-Maestro/maestro-core/actions/runs/36982260107/job/110759181992) | Corrected `otool -L` executable inspection: only `libc++` and `libSystem`; disabled-installer test passes |
| Windows (`windows-latest`) | 32:05 / 33:01 | 5; 0 failed | [three-OS run, Windows job](https://github.com/Orchestration-Maestro/maestro-core/actions/runs/36978566307/job/110747690783) | `objdump -p`: no OpenSSL DLL; disabled-installer test passes |

The first macOS link-inspection selector picked an object file, so its TLS
claim is discarded; the supplemental run inspected the executable and
asserted the expected system-library links. Both disposable pushes used
`test/s2-lbug-repin`; that branch and its workflow were deleted afterwards.
Neither workflow enters the integration branch.

Local Linux: `capped cargo test -p lbug-spike --features engine --locked`
built in 21:06 (1,267.49 s command wall time), with 6 passed and 0 failed.
Both examples build with `--examples`; the reopen test covers on-disk
create/query/read-only reopen. `ldd` reports no OpenSSL, and CMake records
`LBUG_EXTENSION_INSTALLER:BOOL=OFF` without an `OPENSSL_*` entry.

`cargo tree -e normal -i lbug` exits 101 with the following output, proving
that the default build remains lbug-free:

```text
error: package ID specification `lbug` did not match any packages
```

The locked feature-on tree includes lbug. Conservative re-locking
preserves every existing package record except lbug: its new build dependency
edges are `cc`, `libc`, `sha2`, `shlex` and `tempfile`. Only `tempfile` 3.27.0
and its transitive `fastrand` 2.5.0 are new packages; no existing version or
unrelated dependency edge changes.

The organization licence gate passes (advisories, bans, licences, sources).
Imported audits did not cover the two new versions. The qualification ruling
moves the existing revision-specific lbug exemption and adds build-only
`safe-to-deploy` exemptions for those two versions; `imports.lock` is
unchanged. `cargo vet --locked` passes: 32 fully audited, 8 partially audited,
202 exempted. These are exemptions, not new source audits. E07a still waits
for C47a and owns native adapter activation; this spike is not M2 acceptance.

## Original G25 verdict (4301d51)

**Not adopted yet: row 3 (build and cache cost) fails, and stays open until
the organization's CI caches the C++ build.** The other five rows pass.

The probe lands opt-in. `crates/lbug-spike` depends on lbug only through its
`engine` feature, which is off by default, so no default workspace build
compiles the engine. An opt-in probe is not adoption (plan.md A1): no product
crate depends on lbug, and G27 makes it a default dependency only after the
CI change exists.

| # | Check | Result |
| --- | --- | --- |
| 1 | Bundled source | Pass, inside the repository (caveat below) |
| 2 | TLS linkage | Pass |
| 3 | Build and cache cost | **Fail; open** |
| 4 | Platforms and behaviour | Pass: builds and tests on Linux, Windows and macOS; the rest of the behaviour list is ruled into G27 |
| 5 | Licence | Pass; NOTICE obligation assigned to G22 |
| 6 | Dependency cost | Pass |

## What was qualified

- `lbug = { version = "=0.20.4", default-features = false }` in the root
  `[workspace.dependencies]`. crates.io 0.20.4: MIT, MSRV 1.81, edition 2021,
  repository `LadybugDB/ladybug-rust`. Its features are `arrow` and
  `extension_tests`, neither taken.
- `[patch.crates-io]` replaces it with the organization's fork,
  `Orchestration-Maestro/lbug` at `4301d51c13f00f0917653ed753375d5ad37999d1`
  (branch `build/lbug-0.20.4-cmake-reuse`). The fork is the crates.io crate
  imported unmodified (every file hash equal, plus upstream's `LICENSE`), then
  two patches: an optional extension installer (off here, so no OpenSSL) and
  one reused CMake build (ADR-0021).
- `.cargo/config.toml` sets `LBUG_BUILD_FROM_SOURCE`,
  `LBUG_REUSE_CMAKE_BUILD` and `CMAKE_TOOLCHAIN_FILE`
  (`.cargo/lbug-debug-flags.cmake`: the C++ engine's debug builds without
  debug information).
- Toolchains seen: Rust 1.98.1; GCC 13.3 with CMake 3.28-3.31 on Linux;
  MSVC `cl` with CMake 4.4.3 and Ninja 1.13.2 on Windows; Apple clang 17.0.0
  with CMake 4.4.3 on macOS.

The evidence command is `cargo test -p lbug-spike --features engine --locked`.

## Row 1: bundled source

Without `LBUG_BUILD_FROM_SOURCE`, lbug's build script downloads a helper
script from LadybugDB's `main` branch and the latest prebuilt `liblbug`, with
no checksum. `.cargo/config.toml` sets the variable for every build in the
repository, so the build uses the C++ sources bundled in the pinned crate.
The CMake log fetches nothing (the one `FetchContent` sits under
`ENABLE_BACKTRACES`, which is off). In CI, `cargo fetch --locked` took 2 s
on ubuntu, and nothing else is fetched during the build.

**Caveat.** `[env]` in `.cargo/config.toml` applies only when Cargo runs
inside the repository. `cargo install --git`, or `--manifest-path` from
another directory, would take the download path. The fix is a fork change
that makes the source build the default. It is separate work, and it is due
before G27 makes lbug reachable from `maestro`.

## Row 2: TLS linkage and native links

The fork's `extension_installer` feature, off here, gates
`find_package(OpenSSL 3)`, `CPPHTTPLIB_OPENSSL_SUPPORT` and the `ssl` and
`crypto` links. Without it, `INSTALL` fails with "this build of lbug has no
extension installer" and downloads nothing, over HTTPS or plain HTTP.

| OS | Native links of the lbug test binary | Evidence |
| --- | --- | --- |
| Linux | `libstdc++.so.6`, `libgcc_s.so.1`, `libm.so.6`, `libc.so.6`; no `libssl` or `libcrypto` | `ldd`, locally and in run 36509489496; `CMakeCache.txt` has `LBUG_EXTENSION_INSTALLER:BOOL=OFF` and no `OPENSSL_*` entry; test `no_openssl_library_is_loaded` reads `/proc/self/maps` |
| Windows | `MSVCP140.dll`, `VCRUNTIME140.dll`, `VCRUNTIME140_1.dll` and the `api-ms-win-crt-*` runtime; `KERNEL32.dll`, `ntdll.dll`, `WS2_32.dll`, `USERENV.dll`, `bcryptprimitives.dll` and `api-ms-win-core-synch-l1-2-0.dll`; no OpenSSL DLL | `objdump -p` (DLL names), run 36512955122 |
| macOS | `/usr/lib/libc++.1.dylib`, `/usr/lib/libSystem.B.dylib`; no OpenSSL or Homebrew library | `otool -L`, run 36509489496 |

The stock crate links `libssl.so.3` and `libcrypto.so.3` on Linux (route A,
`ldd`).

## Row 3: build and cache cost

**Fail, open.** On every OS, lbug's clean C++ debug build adds over 10
minutes. The organization's gate builds it in four target directories, and
the checks job's timeout is 45 minutes.

### Clean build, per OS

Each job built the base branch and the base plus G25 clean, one after the
other, on the same runner. The command was
`cargo test --locked --workspace --no-run`, the portability leg's build, with
lbug still a default dependency of the spike. Runs are on
`Orchestration-Maestro/maestro-core`; times are minutes:seconds.

| Leg | CPU | Base | With lbug | Added |
| --- | --- | --- | --- | --- |
| ubuntu-24.04 | Xeon 8370C, 4 vCPU | 1:55 | 16:55 | +15:00 |
| ubuntu-24.04 | EPYC 9V45, 4 vCPU | 1:27 | 11:59 | +10:32 |
| windows-2025 | EPYC 7763, 4 vCPU | 4:18 | 30:42 | +26:24 |
| windows-2025 | EPYC 7763, 4 vCPU | 4:27 | 31:05 | +26:38 |
| macos-15 | Apple M1 (virtual), 3 | 2:11 | 12:51 | +10:40 |
| macos-15 | Apple M1 (virtual), 3 | 3:18 | 13:58 | +10:40 |
| ubuntu-24.04 | EPYC 7763, 4 vCPU | 2:06 | 18:10 | +16:04 |
| ubuntu-24.04 | EPYC 7763, 4 vCPU | 2:04 | 17:53 | +15:49 |
| windows-2025 | EPYC 7763, 4 vCPU | 4:25 | 30:36 | +26:11 |
| windows-2025 | Xeon 6973P, 4 vCPU | 3:09 | 19:44 | +16:35 |
| macos-15 | Apple M1 (virtual), 3 | 3:17 | 14:29 | +11:12 |
| macos-15 | Apple M1 (virtual), 3 | 2:11 | 15:06 | +12:55 |

The first six rows are run 36501591821, on `3215f0e`; the last six are run
36505582019, on `7f8f306`, the base this commit lands on. Against the
15-minute bar: macOS passes (+10:40 to +12:55), ubuntu is over in two of
four samples (+10:32 to +16:04), and Windows is over in all four (+16:35 to
+26:38).

Unpaired runs, on different machines: 36499213702 against 36499193190 and
36500536360 (ubuntu +16:39, Windows +13:10, macOS +5:50). Hosted runners of one label differ by up to
half in speed. Earlier builds of the probe alone, in runs 36456020747 and
36459384197, took 9:41 and 14:03 on ubuntu, 18:49 and 15:16 on Windows, and
15:05 and 12:23 on macOS. Run 36450975727 built the same probe with `-g` in
19:22, 23:52 and 9:55.

`cargo --timings` shows the lbug build script as the whole critical path:
11:26 to 16:09 on ubuntu, 18:30 (Xeon 6973P) to 29:28 (EPYC 7763) on
Windows, and 11:55 to 12:55 on macOS. The rest of the workspace finishes
meanwhile.

### The organization's checks job

The job (`ci.yml` `checks`, gate v4.5.0, `timeout-minutes: 45`) builds the
workspace into four target directories:

| Build | Target directory | Added clean, measured on ubuntu |
| --- | --- | --- |
| Clippy and tests | `rust-target/debug` | +10:32 to +16:39 (table above) |
| Coverage | `llvm-cov-target` | +14:30, +15:44 (coverage-shaped: `-C instrument-coverage` in its own target directory) |
| Release tests and build | `rust-target/release` | +20:49, +19:36; +22:42 unpaired |
| Hardening's second release build | `rust-target-verify`, never cached | not measured; the same build as release |

S1's last cold checks job took 22:10 (run 36460443666). With lbug in the
default build, a cold job would take about 90 minutes, and it would time
out. `actions/cache` saves the target directory only when the job succeeds,
so a job that times out never warms the cache. On a warm cache, hardening
still rebuilds the release engine on every run, about +21 minutes, which
breaks the rule that an unrelated change must not rebuild liblbug. This is
why lbug is opt-in.

With the opt-in, default builds (Clippy, tests, coverage, release,
hardening, MSRV and the portability legs) compile no C++ engine. Runs
36509489496 and 36512955122 measured it on this commit, with the same pairs
on one runner:

- the default test build added -1 s and +1 s on ubuntu, -17 s and +4 s on
  Windows, and +27 s and -5 s on macOS (noise);
- the release build added -3 s and -4 s, and the coverage-shaped build -4 s
  and -2 s;
- on all three OSes, `target/debug/build` held no lbug directory after the
  default build.

The features step (`cargo hack check --each-feature`) still builds the
engine once, in debug, when it checks `engine`. A clean
`cargo check -p lbug-spike --features engine` took 13:52 (Xeon 8573C) and
15:29 (EPYC 7763) on ubuntu. That build is cached after a successful run,
and a cold checks job stays under its 45 minutes (about 22 + 16). Building
the probe's tests with `engine` took 12:46 and 16:26 on ubuntu, 13:57 and
9:40 on macOS, and 26:12 and 26:17 on Windows.

### Warm builds, size and memory

- Warm, local: after touching `maestro-kernel`,
  `cargo test --workspace --no-run` took 79 s, and 7 s after touching the
  spike. No C++ was rebuilt. A change of `RUSTFLAGS` rebuilt in 23 s (`-p`)
  and 57 s (`--workspace`), reusing the CMake build.
- Debug `liblbug.a` without debug information: 753 MB (2.6 GB with `-g`).
  The reused build directory is 851 MB after object pruning, and a test
  binary linking lbug is 93 MB.
- Release: linking lbug adds about 19.4 MB to a stripped binary (route A: an
  example of 19.7 MB against a 0.35 MB hello world). Peak RSS of one
  in-memory query: 105 MB.
- Memory: the fork links the archive `-bundle`, so rustc no longer copies it
  into every rlib. A `--workspace` build fits the 8 GiB cap at
  `CARGO_BUILD_JOBS=3`; before that change it was killed twice.
- Not measured: the real-cache cost per mutation shard and per coverage run,
  since the organization's CI does not run on pull requests into
  `feat/s2-integration`.

### What closing row 3 takes

1. A fork change that places the reused CMake build outside the target
   directory, keyed on the rev, flags and target, so coverage and hardening
   reuse the debug and release builds.
2. A rust-workflows change that caches that directory, keyed on the OS, the
   toolchain, `Cargo.lock` and `.cargo/lbug-debug-flags.cmake`, and fills it
   without a cold checks run. The portability legs have no cache today.

Both are separate work (supervisor ruling, 2026-09-28).

## Row 4: platforms and behaviour

**Platforms.** lbug builds natively and every workspace test passes on
ubuntu-24.04, windows-2025 and macos-15 (run 36501591821: 2,316, 2,263 and
2,312 passed, 0 failed, 18 ignored). On this commit, run 36512955122 shows
that default builds compile no lbug on the three OSes, and that
`cargo test -p lbug-spike --features engine` passes: 6 tests on ubuntu, 5
on Windows and macOS. Its workspace tests: ubuntu 2,553 and macOS 2,549
passed, 0 failed. Windows passed 2,492 and failed six S1 tests,
`search::tests::rank_stage::*` (os error 32 when a test scratch directory
is removed while its database is open). That failure has been on S2 since
`7f8f306` (run 36505582019), is S1's, and does not involve lbug.

**Cross-Clippy, lint only.** Clippy never links, but the gate's recipe alone
fails for lbug: `link-cplusplus` looks for a MinGW `g++`, and lbug's build
script would run a full CMake build for the foreign target. `DOCS_RS=1`
makes lbug's build script return at once, and the crate's `env!` needs the
two `LBUG_PRECOMPILED_*` variables:

```sh
export DOCS_RS=1 LBUG_PRECOMPILED_SOURCE=lint-only LBUG_PRECOMPILED_LIBRARY_DIR=
CC_x86_64_pc_windows_gnu=gcc CXX_x86_64_pc_windows_gnu=g++ \
  AR_x86_64_pc_windows_gnu=ar CLIPPY_CONF_DIR="$ORG_CLIPPY_DIR" \
  cargo clippy -p lbug-spike --features engine --all-targets --locked \
  --target x86_64-pc-windows-gnu -- -D warnings
CRATE_CC_NO_DEFAULTS=1 CC_aarch64_apple_darwin="$CC_DARWIN" \
  CXX_aarch64_apple_darwin=g++ AR_aarch64_apple_darwin=ar \
  CLIPPY_CONF_DIR="$ORG_CLIPPY_DIR" \
  cargo clippy -p lbug-spike --features engine --all-targets --locked \
  --target aarch64-apple-darwin -- -D warnings
```

`ORG_CLIPPY_DIR` holds the gate's `clippy.toml`. `CC_DARWIN` is a `gcc`
wrapper that drops `-gfull`, which `ring`'s build script passes on Darwin.
Without `--features engine`, the gate's own recipe is enough. This lints the
Rust code only. Native linking is proven by the CI legs.

**Behaviour found** (`crates/lbug-spike/tests/it/`, asserted on the three
OSes in run 36512955122):

- Create node and relationship tables, a prepared insert with a bound string
  parameter, an edge, an ordered query, close, read-only reopen, the same
  rows and a one-hop query. Writes to a read-only database are refused.
- **File locks differ by OS.** LadybugDB locks the database file with
  `fcntl` on Linux and macOS (advisory, held per process) and with
  `LockFileEx` on Windows (mandatory, held per handle, on the first byte)
  (`src/common/file_system/local_file_system.cpp`, lines 117-160).
  - **A second writer in the same process** opens on Linux and macOS, so
    the adapter must hold one `Database` per path per process. Windows
    refuses it ("Could not set lock on file", error 33).
  - **Another process** is refused a writer on all three OSes. It opens the
    path read-only beside the writer on Linux and macOS. On Windows the
    read-only open fails ("another process has locked a portion of the
    file", error 33): no reader process can open a file a writer holds.
- `INSTALL` fails with the patch's error. G11's Cypher guard must still
  refuse `INSTALL` and `LOAD`: a build with the installer would download
  code at run time.
- `Database` and `Connection` are `unsafe impl Sync`, with 33 `unsafe` sites
  around `cxx`. The workspace's `unsafe` ban covers its own members only.
- Unproven: whether a reader sees consistent data while another process
  writes.

**Ruling: the rest of the behaviour list moves to G27's Red step**
(supervisor, 2026-09-28). This covers:

- Cypher types and bound parameters beyond one string;
- rollback;
- the single parameterized batch loader (`UNWIND $rows`);
- bounded paths filtered at every hop;
- native cancellation or a query timeout;
- a writer beside separate CLI and MCP reader processes. On Windows a
  reader cannot open a file its writer holds, so readers must open only
  published files no writer holds, the intended mode, and the adapter must
  report a clear error otherwise;
- kill and reopen;
- immutable published files with old pins.

G27 proves them on the three OSes through its port, where the adapter's
answer to each (for example, one `Database` per path) is tested. An
unsupported behaviour there fails or blocks, as plan A1 says.

## Row 5: licences

- Rust: `rust-gate licenses` (gate v4.5.0, organization allowlist
  `ISC,BSD-3-Clause,Zlib`) reports advisories ok, bans ok, licences ok and
  sources ok, with the fork allowed as a DEP-001 git source. lbug is MIT;
  the fork keeps upstream's `LICENSE`.
- Bundled C and C++ (cargo-deny does not see it), all permissive:
  - MIT: alp, yyjson, spdlog, cppjieba, brotli, miniz, utf8proc, httplib;
  - BSD: antlr4, re2, snappy, lz4, zstd;
  - Apache-2.0: mbedtls 3.1.0, thrift, parquet, simsimd, fastpfor, roaring.
- **Obligation, assigned to G22 (packaging):** Apache-2.0 requires shipping
  the NOTICE texts of those components with any binary that links lbug.
- Bundled mbedtls is 3.1.0, from 2021. lbug uses it only for SHA-256
  (`src/include/common/sha256.h`, `src/common/sha256.cpp`), and no gate
  scans bundled C for advisories.

## Row 6: dependency cost

- 20 new crates in `Cargo.lock`, plus `lbug-spike`: arrayvec, cmake,
  codespan-reporting, cxx, cxx-build, cxxbridge-cmd, cxxbridge-flags,
  cxxbridge-macro, deranged, foldhash 0.2.0, lbug, link-cplusplus, num-conv,
  powerfmt, rust_decimal, scratch, termcolor, time, time-core,
  unicode-width.
- One forced duplicate: `foldhash` 0.2.0 (cxx) beside 0.1.5 (whatlang 0.18
  through hashbrown 0.15). The DEP-001 exception on 0.1.5 ends when whatlang
  moves to hashbrown 0.17. `clap`, `clap_builder` and `indexmap` show in
  `cargo tree -d` only as feature-set splits at one version.
- `cargo vet --locked` passes (32 fully audited, 8 partially, 200 exempted):
  the new crates are exemptions, and `[policy.lbug] audit-as-crates-io`
  covers the git source. `cargo vet` estimates about 1.41 M lines to review,
  1.30 M of them lbug's vendored C++.

## Deviations from tasks.md

- **A spike crate instead of `N/tests/it/lbug_qualification.rs`.** No
  product crate gains lbug before a verdict, so the probe is
  `crates/lbug-spike`, with its own `it` test binary. That is one more
  native-linked test binary than TST-003 wants, built only with `engine`.
  G27's probes belong under `crates/maestro-knowledge/tests/it/`, and the
  spike goes when they exist.
- **No `.github/workflows/lbug-qualification.yml`.** The organization's
  portability legs run only on pull requests into the default branch. G25
  used a temporary workflow on draft pull requests #53 and #55 instead
  (supervisor ruling, 2026-09-28 12:25), and dropped it before landing.
- **The behaviour list is partly probed** (row 4 ruling).
- **Build and cache cost fails** and is not waived (row 3).

## Constitution Check recheck

The plan's Constitution Check, rechecked against this verdict. G24 performs
the final recheck.

| Rule IDs | After G25 |
| --- | --- |
| C-001 | Rechecked here. Blocker: row 3, until the CI change exists. |
| C-006, DEP-001 | Two named exceptions with a forcing library and a removal condition: the fork as a git source (until an upstream release makes OpenSSL optional) and `foldhash` 0.1.5. No gate exception. |
| ENF-002 | Working lint-only cross-Clippy recipes (row 4). Native builds and tests pass in CI on the three OSes; G22 and G24 still require them for the product. |
| ENF-012 | Engine pinned by crate version, fork rev (full hash in `Cargo.lock`) and features. The prebuilt download is off inside the repository (row 1 caveat). |
| TST-001, TST-003 | Deviation: the spike's own opt-in test binary. G27's tests go into `maestro-knowledge`'s `tests/it`. |
| COV-001, COV-002 | Default coverage does not build the engine; the spike's library holds no code, and its tests run only with `engine`. G27's adapter falls under the normal thresholds. |
| SEC-003 | Unchanged: G26 still rejects caller-supplied engine paths; G11's guard refuses `INSTALL` and `LOAD`. |

## Sources

- The supervisor's G25 reports and review (route A, route B, g25b and g25c)
  hold the full command logs.
- CI runs 36450975727, 36456020747, 36459384197, 36499193190, 36499213702,
  36500536360, 36501591821, 36505582019, 36509489496 and 36512955122 on
  `Orchestration-Maestro/maestro-core`.

## G26: packaging and read-only local diagnostics

G26 adds the setting `graph.engine` (`none` by default, or `lbug`) and the
`maestro` crate's `engine` feature, without which `lbug` is refused by setup
and reported by health. The feature holds no engine code yet: the lbug adapter
of the health port, and a CI job that builds `--features engine` to run its
tests and mutants (as `mutation-windows` does for Windows code), belong to
G27. Default builds therefore still compile no C++.

- **Setup** previews, and with `--yes` creates, `<data directory>/graph`,
  mode `0700` on Unix, on every platform, before the unchanged Qdrant part.
  It downloads nothing, opens no engine and removes nothing.
- **Health** (`status`, `doctor`) is read-only. It reports the graph off with
  no probe, the engine missing from the build, the directory missing, shared,
  relocated (a link) or not a directory, then asks the `PublishedGraph` port
  for the receipt's files. Each file must lie inside the graph directory with
  no link on the way before `PublishedFile::open_read_only` is called; it is
  opened read-only, queried with `RETURN 1`, closed, reopened and queried
  again. An open refused with "lock" in the engine's message is a writer's
  lock (G25's messages on Windows); any other refusal, or a failed query, is
  a corrupt or unreadable file to rebuild. The production adapter publishes
  nothing until G27 writes the receipt, so an enabled check stops at "no graph
  is published yet" with zero opens. Tests drive every state with fakes.
- **Cleanup**: no command. Offline rebuild and reader-safe removal are
  documented in `docs/how-to/knowledge-graph.md`. The Red item "cleanup
  refuses live readers and non-owned files" moves to G27: before its receipt
  nothing says which files are the graph's, and on Linux and macOS the
  advisory lock cannot keep a reader out.

### Measurements

Linux x86-64 (WSL2, 8 threads), 2026-09-29, network off: every run below ran
in `unshare -rn`, a network namespace holding only a down loopback. The engine
numbers come from the lbug-spike crate's engine build (the C++ debug build of
`.cargo/lbug-debug-flags.cmake`, the only one this machine reuses), through
`examples/open_reopen.rs` on a scratch database of 1,000 nodes; three runs.

| Measure | Result |
| --- | --- |
| Install (`maestro --set graph.engine=lbug setup --yes`, engine-feature build) | under 10 ms wall per run (`time` reports 0.00 s), 18.4-18.6 MB peak RSS; the directory created `drwx------`; no download, no engine open |
| Create 1,000 nodes and close | 4.8-5.8 s (one query per node) |
| Open read-only | 56-68 ms; with `RETURN 1`: 64-77 ms |
| Reopen read-only | 54-57 ms; with `RETURN 1`: 55-58 ms |
| Disk | one file, `graph.lbug`, 2,142,208 bytes; no WAL left after close |
| Peak RSS of the measuring process | 118.4-119.0 MB (G25: 105 MB for one in-memory query) |
| Binary size | debug example 86.4 MB, 60.8 MB with debug sections stripped; `maestro` itself links no engine until G27. G25's release measure stands: lbug adds about 19.4 MB to a stripped binary |

Commands: `capped cargo build -p lbug-spike --features engine --example
open_reopen --locked --offline`, then `unshare -rn /usr/bin/time -v
target/debug/examples/open_reopen <empty scratch directory>`. The logs are in
the ledger's `.superpowers/sdd/s2-knowledge-graph/g26-evidence/`, outside this
repository. The `graph_operations` tests ran in the same namespace, in the
default and the `engine` builds.
