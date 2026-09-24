set shell := ["bash", "-euo", "pipefail", "-c"]

local_bin := justfile_directory() / ".tools/bin"
export PATH := local_bin + ":" + env_var("PATH")

# List every recipe and what it does; this is what `just` alone prints.
help:
    @just --list --unsorted

# Install the pinned toolbelt and link it into ignored .tools/bin (Linux x64).
setup:
    #!/usr/bin/env bash
    set -euo pipefail
    [[ "$(uname -s)" == Linux && "$(uname -m)" == x86_64 ]] || {
      echo 'Pinned tooling supports Linux x64 only' >&2; exit 1;
    }
    # mise installs every tool mise.toml pins and refuses bytes that differ
    # from mise.lock; scripts/bootstrap.sh verified mise itself.
    mise trust mise.toml
    mise install --locked
    mkdir -p .tools/bin
    find .tools/bin -mindepth 1 ! -name mise -delete
    while IFS= read -r directory; do
      for file in "$directory"/*; do
        if [[ -f "$file" && -x "$file" ]]; then
          ln -s -- "$file" ".tools/bin/$(basename -- "$file")"
        fi
      done
    done < <(mise bin-paths)
    prek install

# The local gate: CI's checks and the repository policies. Run before a push.
check:
    #!/usr/bin/env bash
    set -euo pipefail
    for tool in mise just actionlint zizmor yamlfmt taplo shellcheck prek cargo rustup \
      gitleaks typos jaq cargo-deny cargo-llvm-cov cargo-machete; do
      command -v "$tool" >/dev/null ||
        { echo "Missing $tool; run scripts/bootstrap.sh" >&2; exit 1; }
    done
    just --unstable --fmt --check
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo test --workspace --locked
    just docs
    cargo llvm-cov --workspace --locked --fail-under-lines 90 --summary-only
    cargo deny --offline check licenses bans sources
    cargo machete
    actionlint
    zizmor --offline --persona=pedantic --no-progress --config .github/zizmor.yml .github/
    yamlfmt -no_global_conf -lint
    taplo fmt --check
    typos
    # Secrets in every file a commit could take, target/ and .tools/ aside.
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
    cargo test --locked -p maestro-canonicalization --test chunk_native -- --ignored

# Every mutant in the workspace; a survivor fails. Hours long: run before an import.
mutants jobs="4":
    cargo mutants --workspace --jobs {{ jobs }} --cargo-arg=--locked
