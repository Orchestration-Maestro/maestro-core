set shell := ["bash", "-euo", "pipefail", "-c"]

# The toolbelt `rust-gate setup` installs, at the versions CI runs, ahead of
# whatever else the PATH carries.
unix_cache := env('XDG_CACHE_HOME', home_directory() / '.cache')
cache_home := if os_family() == "windows" { env('LOCALAPPDATA') } else { unix_cache }
tools_bin := cache_home / 'maestro/tools/bin'
path_sep := if os_family() == "windows" { ";" } else { ":" }
export PATH := tools_bin + path_sep + env_var("PATH")

# List every recipe and what it does; this is what `just` alone prints.
help:
    @just --list --unsorted

# The local gate: CI's checks job, step by step, in CI's environment, over the
# commits a push sends. The pre-push hook runs the same command.
check:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v rust-gate >/dev/null ||
      { echo "Missing rust-gate; install it, then run rust-gate setup" >&2; exit 1; }
    rust-gate ci --local

# Build the API documentation with every item documented and warnings denied.
docs:
    RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked --document-private-items

# The native tokenizer tests; MAESTRO_NATIVE_BINDING names this machine's binding.
native:
    cargo test --locked -p maestro-canonicalization --test it chunk_native -- --ignored

# Every mutant in the workspace; a survivor fails. Hours long: run before an import.
mutants jobs="4":
    cargo mutants --workspace --jobs {{ jobs }} --cargo-arg=--locked
