# C09 dependency measurements

**Measured:** 2026-09-29, on `feat/s3-c09`; no workspace library, lockfile,
`maestro-quality.toml`, vet or DEP-001 entry was changed. Scratch package:
`c09-measure` (unpublished, MIT license solely to keep cargo-deny output
focused).

## Selected dependencies and features

Complete selected scratch manifest:

```toml
[package]
name = "c09-measure"
version = "0.1.0"
edition = "2024"
license = "MIT"

[dependencies]
tar = { version = "0.4", default-features = false }
jsonschema = { version = "=0.58.2", default-features = false }
cedar-policy = { version = "=4.13.0", default-features = false }
```

Selected: `tar 0.4.46`, `jsonschema 0.58.2`, `cedar-policy 4.13.0`.
`tar` defaults off, so it does not enable `xattr`; bundle construction uses
archive entries/bytes only. `jsonschema 0.58.2` is the latest measured 0.58
release and implements draft 2020-12. Its default resolver, file and HTTP
features are off; schemas cannot initiate local or remote reference loads.

The 0.58.2 scratch resolution contains 198 registry packages (199 lock
entries including the scratch package), compared with 215 registry packages
(216 lock entries) for 0.30.0. The 0.58.2 Cargo.lock SHA-256 is
`d53e9dc41e9194e1d93963e75c45814e951fbe7dbb17c4093b97598880a7dab6`; the
0.30.0 comparison lock SHA-256 is
`cd0abed039758646464b25e77da23b55c39d168072b2857a8a2940619b6fc6a3`.
The selected Cargo.toml SHA-256 is
`c35fc51723e0952138c24353b9492439d5a84a4596977c28f9ce37ddeaefbd8e`.
The smaller measured closure and absence of defaults select 0.58.2 over 0.30.0.

With 0.58.2 default features disabled, `idna` is absent from the active
closure. The library gates `idn-hostname` and `idn-email` validation behind
its `idna` feature; without it, those format names are unknown and are not
asserted. No current workspace schema uses either format (current schema
formats are `uint64`, likewise an application-specific format). The selected
schemas therefore do not require `idna`; C19 must enable and test `idna` if a
schema begins to assert `idn-hostname`/`idn-email`. This is an explicit feature
boundary, not an assertion that those formats validate without it.

`cedar-policy` public package features are empty with defaults disabled, but
its 4.13.0 manifest does not disable defaults on its `cedar-policy-core`
dependency. The core consequently enables `datetime`, `decimal` and `ipaddr`
anyway; they cannot be removed by this consumer without changing the approved
library. Experimental, protobuf, wasm, async and heap-profiling features stay
off.

## Closure, licenses and native builds

Run the gate v4.6.0 policy against the scratch manifest using the generated
organization config, with `LICENSE_ALLOWLIST=ISC,BSD-3-Clause,Zlib`:

```sh
cargo deny --manifest-path "$C09_MANIFEST" --config "$C09_DENY_CONFIG" check licenses advisories sources
```

The same full-closure check on the 0.30.0 comparison and selected 0.58.2 both
reports **only** `ar_archive_writer 0.5.3` (`Apache-2.0 WITH LLVM-exception`)
as a rejected license. It enters through `(build) psm 0.1.32 → stacker 0.1.25
→ cedar-policy-core 4.13.0`. For both, advisories and sources pass. The top-level
licenses `tar: MIT OR Apache-2.0`, `jsonschema: MIT`, and `cedar-policy:
Apache-2.0` are allowed by the gate's built-in policy. Exact OA4 maintainer ask:
add `Apache-2.0 WITH LLVM-exception` to `LICENSE_ALLOWLIST` before C19 adopts
Cedar. This task changed no organization variable.

`cargo tree -i cc -e normal,build --target all` records Cedar's target build
cost: `cedar-policy-core → stacker 0.1.25 → psm 0.1.32 → cc 1.5.1` (`cc` is
a build dependency; psm assembles target-specific code). The `psm` build chain
must cross-compile/lint on each supported target. C19 must check cross Clippy
at adoption, in particular Windows GNU and macOS ARM.

The current workspace's duplicate package versions include base64, getrandom,
hashbrown and syn. Candidate integration introduces these two forced pairs:

- `itertools 0.14.0` is required by `lalrpop 0.22.2` (Cedar core build);
  `itertools 0.15.0` is used by Cedar crates. Remove the duplicate when
  `lalrpop` uses `itertools 0.15.0`.
- `unicode-width 0.1.14` is required by `miette 7.6.0`; `unicode-width 0.2.2`
  is required by `pretty 0.12.5` under `cedar-policy-formatter`. Remove the
  duplicate when `miette` uses `unicode-width 0.2.2`.

At adoption, C10/C19/C22b must measure the integrated lock and add DEP-001
entries only for pairs that remain, with these named removal conditions. The
scratch closure has `defmt` and `wasm-bindgen-shared` Cargo `links` metadata;
there is no new C library/system `links` entry. This metadata is not a complete
native-build audit: the `cc` path above is separately measured. Workspace
`cargo vet --locked` passed on the unchanged lock (32 fully audited, 8
partially audited, 181 exempted); it is not a vet verdict for unadopted
scratch dependencies. Adoption owners run integrated vet and register their
own records then.

## Reproduction

`$C09_MANIFEST` is the selected complete manifest above. The 0.30 comparison
changes only `jsonschema` to `=0.30.0`, with the same package stanza and other
dependencies. Scratch locks and JSON metadata are kept outside the repository;
locks are identified by the digests above. Example local wrapper form (the
`capped` binary and toolbelt PATH are optional machine-local wrappers):

```sh
PATH="$HOME/.cache/maestro/tools/bin:$PATH" CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 ~/.local/bin/capped cargo tree --manifest-path "$C09_MANIFEST" -e features
PATH="$HOME/.cache/maestro/tools/bin:$PATH" CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 ~/.local/bin/capped cargo tree --manifest-path "$C09_MANIFEST" -d
PATH="$HOME/.cache/maestro/tools/bin:$PATH" CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 ~/.local/bin/capped cargo tree --manifest-path "$C09_MANIFEST" -i cc -e normal,build --target all
PATH="$HOME/.cache/maestro/tools/bin:$PATH" CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 ~/.local/bin/capped cargo metadata --manifest-path "$C09_MANIFEST" --format-version 1
cargo deny --manifest-path "$C09_MANIFEST" --config "$C09_DENY_CONFIG" check licenses advisories sources
```

Plain Cargo equivalents, for environments without the optional local
`~/.local/bin/capped` wrapper/toolbelt PATH:

```sh
cargo tree --manifest-path "$C09_MANIFEST" -e features
cargo tree --manifest-path "$C09_MANIFEST" -d
cargo tree --manifest-path "$C09_MANIFEST" -i cc -e normal,build --target all
cargo metadata --manifest-path "$C09_MANIFEST" --format-version 1
cargo deny --manifest-path "$C09_MANIFEST" --config "$C09_DENY_CONFIG" check licenses advisories sources
```

The lane ran every cargo command through `capped` with
`CARGO_BUILD_JOBS=3` and `CARGO_INCREMENTAL=0`; the wrapper and toolbelt PATH
are optional local conveniences, not project requirements.
