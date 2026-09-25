set shell := ["bash", "-euo", "pipefail", "-c"]

# The toolbelt `rust-gate setup` installs, at the versions CI runs, ahead of
# whatever else the PATH carries.
unix_cache := env('XDG_CACHE_HOME', home_directory() / '.cache')
cache_home := if os_family() == "windows" { env('LOCALAPPDATA') } else { unix_cache }
tools_bin := cache_home / 'maestro/tools/bin'
export PATH := tools_bin + ":" + env_var("PATH")

# List every recipe and what it does; this is what `just` alone prints.
help:
    @just --list --unsorted

# `rust-gate ci --local` replaces this recipe's body once rust-workflows ships it;
# until then CI's dependency-policy step alone checks licences, bans and sources.

# The local gate: CI's checks and the repository policies. Run before a push.
check:
    #!/usr/bin/env bash
    set -euo pipefail
    for tool in mise just actionlint zizmor yamlfmt taplo shellcheck prek cargo rustup \
      gitleaks typos jaq cargo-llvm-cov cargo-machete rust-gate; do
      command -v "$tool" >/dev/null ||
        { echo "Missing $tool; run rust-gate setup" >&2; exit 1; }
    done
    just --unstable --fmt --check
    cargo fmt --all --check -- --config style_edition=2024
    rust-gate clippy --local
    cargo test --workspace --locked
    just docs
    cargo llvm-cov --workspace --locked --fail-under-lines 90 --summary-only
    cargo machete
    actionlint
    zizmor --offline --persona=pedantic --no-progress --config .github/zizmor.yml .github/
    yamlfmt -no_global_conf -lint
    taplo fmt --check
    typos
    # Secrets in every file a commit could take, target/ aside.
    tree=$(mktemp -d)
    trap 'rm -rf "$tree"' EXIT
    while IFS= read -r -d '' file; do
      if [[ -f "$file" ]]; then cp --parents -- "$file" "$tree/"; fi
    done < <(git ls-files -z --cached --others --exclude-standard)
    gitleaks dir --no-banner --redact "$tree"
    [[ "$(git ls-files | wc -l)" -gt 0 ]] ||
      echo 'NOT RUN: the commit hooks; git tracks no file yet, so every hook skips'
    prek run --all-files

# Build the API documentation with every item documented and warnings denied.
docs:
    RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked --document-private-items

# The native tokenizer tests; MAESTRO_NATIVE_BINDING names this machine's binding.
native:
    cargo test --locked -p maestro-canonicalization --test it chunk_native -- --ignored

# Every mutant in the workspace; a survivor fails. Hours long: run before an import.
mutants jobs="4":
    cargo mutants --workspace --jobs {{ jobs }} --cargo-arg=--locked
