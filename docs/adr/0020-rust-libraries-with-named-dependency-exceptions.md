# Rust libraries join the stack; the duplicates they force are named exceptions

Status: accepted, 2026-09-26; S6 browser-role amendment accepted, 2026-09-30.

The S6 amendment below supersedes the original acquisition/conversion shortlist
where it differs. Approval of a direction is not dependency adoption or platform
qualification; this amendment changes no workspace dependency or policy file.

Maestro builds on the Rust ecosystem's libraries rather than on hand-written
clients: `qdrant-client` for the search projections (ADR-0003), `neo4rs` for
the graph projection (ADR-0004), `docling` (docling.rs) for document
conversion, `spider` and `dom_smoothie` for acquisition. The organization's
one-version rule (DEP-001) stays the default. The ecosystem is midway through
several major transitions (`syn` 2 to 3, `getrandom` 0.2 to 0.3,
`windows-sys`), so an adopted library can force a second version of a crate
maestro-core already uses. Each such duplicate is a DEP-001 exception in
`maestro-quality.toml` naming the crate and version, the library that forces
it and the version that ends it, and it is removed when the library moves.

Before a library is adopted, its tree is measured against the workspace's
lock: the crates it adds, the second versions it forces, the native libraries
it links. It is taken with the fewest features it needs, and the numbers go
into the task's brief. A library whose native link clashes with the core's is
taken only with features that avoid the clash.

Measured on 2026-09-26 against maestro-core's 128 crates:

| Library | Crates in its tree | New to maestro-core | Second versions it forces |
| --- | ---: | ---: | --- |
| `qdrant-client` 1.19.0, no default features | 128 | 78 | 4: `syn`, `base64`, `getrandom`, `windows-sys` |
| `neo4rs` 0.9.0-rc.10 | 122 | 84 | 4: `syn`, `getrandom`, `socket2`, `windows-sys` |
| `dom_smoothie` 0.18.2 | 61 | 45 | 1: `syn` |
| `docling` 1.69.2, no default features to defaults | 146 to 439 | 120 to 363 | 9 to 23 |
| `spider` 2.53.9, no default features to defaults | 268 to 424 | 186 to 331 | 15 to 38, a second `reqwest` among them |

`spider`'s default features also link a second SQLite (`libsqlite3-sys` 0.30
beside the kernel's 0.38), which no build can link; without them it links
none.

## S6 amendment: crawl4ai browser-render-only exception

**Owner decision, 2026-09-30:** Rust owns acquisition producers, connectors and
extraction. The sole Python exception is **crawl4ai, out of process**, only where
JavaScript or Chromium is unavoidable, behind the replaceable admitted fetch/render
port. It replaces Spider's chromey route. Spider remains approved with
`default-features = false`, extras, `chrome` and `chromey` off, but is unused
by the current plan. No chromey,
chromiumoxide, WebDriver or other production browser fallback is approved.

The exception permits bounded browser requests/rendered DOM capture only:
**no frontier** or independent crawl/discovery authority, **no conversion** (including
Markdown extraction), **no model** execution/download, and **no publication** or
indexing role. Rust core policy admits every destination and validates returned
captures. Configured readiness is bounded and declarative, not arbitrary source
scripts or network-idle alone. A disabled, missing or unqualified adapter refuses;
its callers do not install or select another browser stack.

### Captured link reading: dom_query

**Owner decision, 2026-10-01:** link reading uses `dom_query` 0.28.0 (MIT),
the `^0.28.0` reader in the approved `dom_smoothie` 0.18.2 family. It reads
already verified N09/N12 captures offline; it has no network client, crawl
queue or grant authority. CSS selection is available without default features;
Markdown conversion, optional hashbrown and mini-selector features stay off.
The checked-in selector list includes hidden anchors and the declared resource
attributes, honours the first `<base href>`, and participates in the immutable
extractor contract digest. Relative URLs use the existing `url` 2.5.8 crate.

Against N57's integration lock, adoption adds 30 packages and one new duplicate
family: foldhash 0.1.5/0.2.0. Its DEP-001 exception names whatlang's older
hashbrown dependency and the foldhash 0.2 exit. There is no new native link,
HTTP client, crypto stack or SQLite. Cargo-vet entries for this closure are
explicit deployment exemptions, not completed security audits. Servo CSS
components use the already-allowed MPL-2.0 licence; other added terms are
MIT, Apache-2.0 and Zlib. Spider's measured default-off candidate added 112
packages, a second reqwest and native crypto/compression; it was not adopted.

### Artifact and licence record

N02 inspected the public **crawl4ai 0.9.4** release, not an installed deployment.
The candidate PyPI wheel `crawl4ai-0.9.4-py3-none-any.whl` was downloaded only
for archive inspection, not installed; its verified SHA-256 matches the registry:
`46779c8a93b35c7c5f9be9642ddef5e7b515e36b094639bb8e1e418bf0266179`.
The source distribution has registry SHA-256
`fd118861b1acff90ac0db4ceae4d529691fd1a0a0d4417c710de3c7d2114ae88`.
These identify audit candidates, not approved production artifacts. Before launch,
the adapter declaration must bind exact adapter/wheel, Python interpreter,
transitive wheels/native assets, browser automation runtime and Chromium build
versions/digests, protocol, configuration, supported OS/architecture and licence
receipts. PyPI's `Python >=3.10`, `playwright >=1.49.0` and
`patchright >=1.49.0` are **not closure pins**. The exact Python/browser closure,
ABI compatibility and platform evidence remain blockers; no version is invented.

PyPI reports Apache-2.0, but the [release licence](https://github.com/unclecode/crawl4ai/blob/v0.9.4/LICENSE)
also has an **Attribution Requirement**. The fetched text and the wheel's embedded licence both have SHA-256
`193fe32704bee15cd74f5153e569cdf830e26445e19798a55daee32da40758aa`.
It requires this notice in a prominent, accessible location, including CLI help:

> This product includes software developed by UncleCode (<https://x.com/unclecode>)
> as part of the Crawl4AI project (<https://github.com/unclecode/crawl4ai>).

Review the actual wheel/source terms, notice placement and every Python/native/
browser component before distribution or use. The SPDX label alone does not
settle the additional terms; that review remains a blocker. The mandatory Python
requirements include extraction/LLM packages even with extras off; their presence
grants no conversion/model capability and their full closure still needs audit.

### Controls, related shortlist and removal

All adapter/runtime/browser assets are provisioned in advance, pinned and verified;
**no downloads** at build, launch, first use or recovery. Do not invoke
`crawl4ai-setup`, browser installers, model-download commands or the upstream live
`doctor` probe. Upstream setup installs both Playwright and Patchright Chromium;
it is not the qualified single-route launcher. Browser traffic is allowed only
through current core admission, including redirects/subresources and otherwise
bypassing channels. N46 must prove checked-address egress, credential isolation,
sandbox/resource bounds, readiness, cancellation and owned-process-tree reaping
on each supported platform; process separation alone is not containment. Missing
controls hold that platform/adapter rather than enabling an unsafe fallback.

The approved bake-off is unchanged: **Xberg 1.3.0** minimal native PDF/Rust
layout/OCR versus native **docling 1.78.0**, no defaults plus `pdf-text`.
Python Docling remains comparison-only. **tract is the layout/OCR default;
dynamic ONNX Runtime is table-only**, with offline, digest-pinned, licence-checked
weights/runtime assets. Never enable `download-binaries`: the published Xberg
ORT and docling ML feature edges still require an upstream split or an explicitly
approved audited patch. Enabling dynamic loading does not subtract download
features. Optional native/model exceptions, unselected libraries and every patch
remain individually owner-approval-blocked.

Remove this Python exception when a **qualified Rust browser adapter** meets the
same transport, readiness, session, containment, offline-artifact and three-platform
controls, and the owner approves its measured dependency/artifact record. Replace
the adapter behind the port, remove the Python/browser closure that is no longer
needed, and retire the exception with its qualification evidence. Until then,
no alternative library is implicitly approved.

N02's [current-lock measurements and blockers](../../specs/006-native-acquisition/research.md#39-n02-measured-dependency-probe)
use S6 integration `e16cad0f82e329d6a3971379078cc2560d00d7d8`: **229 packages,
221 registry packages**. The 128-crate table above remains explicitly historical.
Every later addition repeats the probe against its then-current lock and records
each forcing library/version, licence, native link and duplicate-removal condition
before any DEP-001/vet record or dependency adoption.

## S6 amendment: N17 Linux parser containment

The owner approved Option A and its scoped host setup on **2026-10-02**
("yes go"). This adapter uses nix namespaces instead of architecture 03's
bubblewrap route, behind the replaceable isolation port. It conveys no source,
grant, extraction, publication, private-data or platform authority.

Exact additions, defaults off: `landlock =0.4.7`, `seccompiler =0.5.0` (no JSON
frontend), and `nix =0.31.3` with only `fs,mount,process,sched` (implicit `uio`).
Existing `rustix =1.1.5` adds `thread`. Against S6 `51c56a2`, the lock moves from
**260 to 266 packages**: six additions, no removals/upgrades, no new duplicate
families or Cargo native `links`. DEP-001 needs no new duplicate exception.
Cargo-vet's named exemptions are adoption records, not completed security audits.

| Package | Licence | Registry checksum (SHA-256) |
| --- | --- | --- |
| cfg_aliases 0.2.2 | MIT | `f079e83a288787bcd14a6aea84cee5c87a67c5a3e660c30f557a3d24761b3527` |
| enumflags2 0.7.12 | MIT OR Apache-2.0 | `1027f7680c853e056ebcec683615fb6fbbc07dbaa13b4d5d9442b146ded4ecef` |
| enumflags2_derive 0.7.12 | MIT OR Apache-2.0 | `67c78a4d8fdf9953a5c9d458f9efe940fd97a0cab0941c075a813ac594733827` |
| landlock 0.4.7 | MIT OR Apache-2.0 | `4cca98e95f35b29d469dade6724c6f96cec9236640f745a0e99b0334ec320ab1` |
| nix 0.31.3 | MIT | `cf20d2fde8ff38632c426f1165ed7436270b44f199fc55284c38276f9db47c3d` |
| seccompiler 0.5.0 | Apache-2.0 OR BSD-3-Clause | `a4ae55de56877481d112a559bbc12667635fdaf5e005712fd4e2b2fa50ffc884` |

Mandatory controls: user/mount/PID/network/IPC/UTS namespaces; a private immutable
filesystem; strictly fully enforced Landlock V3+; capability removal and
`no_new_privs`; x86-64 default-deny seccomp; delegated cgroup-v2 memory (swap 0),
CPU and PID ceilings with read-back; finite parent output/deadline budgets from
OA3/composed envelopes; one cumulative N09/N16 IPC decode ledger. Missing controls
refuse, with no unsupported-host fallback. Windows/macOS remain N18/N19.

The executable closure is the pinned parser/bootstrap and the digest-pinned ELF
interpreter declared by the parser. Libraries are copied from explicit root-owned,
non-group/world-writable read-only handles into the immutable view; no ambient
library directories are mounted. Only parser/interpreter gain Landlock Execute.
Input and scratch are noexec, with no Execute right. Loader environment is empty.
Seccomp additionally forbids anonymous/RWX executable mappings, adding Execute by
mprotect, memfd creation and other executable-memory/management bypasses.

Every initial parser exec uses a sealed digest-checked memfd. Descendants may
re-exec only the core-written `/parser` snapshot, digest-verified after writing
and before launch, under the read-only root mount and Landlock. They cannot
write, rename or replace it. The supervisor approved this closure rather than
adding procfs (which would reopen executable-memory bypasses); Linux refuses a
bind mount of the anonymous memfd inode. No fallback or procfs is added.
Explicit bootstrap mode has no default: WSL without AppArmor userns restrictions
uses a sealed copy; installed
Ubuntu mode opens/hashes one regular executable root-owned, non-group/world-writable
inode and executes its FD, preserving launcher-bound AppArmor attachment. CI
asserts actual attachment when the scoped profile was needed.

This machine uses a per-run user systemd scope with `Delegate=cpu memory pids`.
The trusted supervisor/ancestors occupy a manager leaf outside bounded workers.
Ubuntu CI's reviewed provisioner creates transient non-root runner services with
that delegation, `KillMode=control-group` and their own memory cap. An application-
scoped `userns,` profile attaches only to the root-owned verified bootstrap under
`/opt/maestro/n17/<digest>/bootstrap`, only when its namespace probe requires it.
No global AppArmor/sysctl disable, privileged parser or native-helper exception.
The provisioner removes its units, profile and installed image after qualification.

Success, refusal, crash, timeout and cancellation all kill/reap the owned tree,
remove its leaf/scratch and close memfds before any output is released. On trusted
supervisor death, systemd collects the owned tree/unit; next start reclaims only
owned scratch with an absent/empty recorded cgroup. Active receipts refuse. A
locked receipt-only preparing directory is atomically renamed with no-replace
before snapshots or workers exist; recovery deletes it only after obtaining its
nonblocking exclusive lock. Live preparers are preserved. Cleanup failures hold.

Default tests assert the syscall, filesystem, namespace and cgroup plans as data
and apply actual unprivileged Landlock/seccomp in a child without namespace setup.
Privileged host-only mutation coverage remains a pre-S6-main obligation; baseline
qualification is not mutation evidence.

WSL and hosted Ubuntu are qualified separately by the real `n17_` tests, not by
kernel versions, cross-compilation or dependency probes. CI uploads exact tested
SHA, lock/image hashes, kernel/ABI and denied-effect/cleanup receipts. Retire this
stack only after an owner-approved replacement behind the port proves the same
controls/host setup; remove its unused dependency closure then.

## Considered options

- Hand-written HTTP clients, T026's first design: no duplicates, but maestro
  would maintain protocol code the vendors already maintain, for every
  service. Rejected.
- Relaxing DEP-001 for the whole organization, or for build-only crates such
  as `syn`: fewer exceptions, but the rule's readers would no longer see which
  library holds which duplicate. The build-only case, procedural-macro
  dependencies that ship in no binary, is worth proposing to the gate's
  maintainers; until the gate distinguishes it, exceptions.
- Every large library in its own process, ADR-0013's extension model: it
  isolates crashes and untrusted input at the cost of a protocol per library.
  Kept as a choice each adoption weighs, for example a crawler or a PDF parser
  reading untrusted input, not as a rule.

## Consequences

`maestro-quality.toml` gains an exception for each forced duplicate, with its
removal condition, and `cargo vet` gains an exemption or an audit for each new
crate. Builds grow with each library, since a second `syn` compiles twice;
binaries grow less, since build-only duplicates ship in none. Upgrading a
library is a task: measure again and drop the exceptions it no longer forces.
