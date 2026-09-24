# Foundation Implementation Tasks

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task by task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** a public `maestro-core` whose CI passes the organization's gates with
only the canonicalization crate, plus the repositories S1 needs.

**Architecture:** one Cargo workspace; the crate inherits the workspace lints,
its oversized files and functions are split without changing behaviour, the
native tokenizer reads machine paths from a local binding, and the repository's
policies are tests. See [plan.md](plan.md) for the decisions (D1–D8).

**Tech Stack:** Rust 1.98.1, Cargo, just, mise, prek, rust-workflows v1.2.1,
Spec Kit 1.0.1, llama.cpp `llama-tokenize`.

**Spec:** [spec.md](spec.md) · **Plan:** [plan.md](plan.md)

## Global Constraints

- Rust 1.98.1, edition 2024; the crate's declared MSRV stays 1.85; no new
  crates.io dependency.
- Every member manifest has `[lints]` `workspace = true` and no `[lints.*]` table;
  `clippy.toml` keeps complexity 15, 100 lines, 5 parameters.
- Files at most 500 counted lines; Rust, shell and Just lines at most 100
  columns; commit subjects conventional, lower case after the type, at most 71
  characters, lines at most 80.
- `maestro-canonicalization`'s public API is unchanged: names, signatures,
  `SCHEMA_VERSION` `1.2.0`, `PARSER_VERSION`, identity recipes.
- Fixture bytes are unchanged: `sha256sum --check` against the baseline of T001
  passes after every task.
- No personal path, secret or vendor-private material in any file that can be
  pushed; the archive and the binding live outside the repository.
- English for code and repository prose.
- Local commits go on `work/foundation`, which is never pushed; T022 publishes
  one clean commit.
- `just check` exits 0 before anything is pushed.

`ARCHIVE` below is `~/archives/maestro-core-2026-09-24`, `CRATE` is
`crates/maestro-canonicalization` (the crate is renamed in T002 Step 1; before
that it is `crates/document-canonicalization`) and `RW` is
`~/workspace/Orchestration-Maestro/rust-workflows`. A step that says
**Commit:** with a title runs `git add -A && git commit -m "<title>"`.

---

## Phase 1: Setup

### T001 Archive the working tree and record the baseline [US1]

**Files:** none in the repository; creates `ARCHIVE/`.

- [x] **Step 1: Confirm with the maintainer.** The archive precedes deletions in
  T002. Ask: "Archive maestro-core to ~/archives/maestro-core-2026-09-24, then
  remove docs/superpowers, docs/providers and the crate's leftovers?" Stop on
  anything but yes.

- [x] **Step 2: Write the archive**

```bash
cd ~/workspace/Orchestration-Maestro
ARCHIVE=~/archives/maestro-core-2026-09-24
mkdir -p "$ARCHIVE"
tar --create --gzip --file "$ARCHIVE/maestro-core.tar.gz" \
  --exclude=maestro-core/target --exclude=maestro-core/.tools \
  --exclude=maestro-core/crates/document-canonicalization/target maestro-core
sha256sum "$ARCHIVE/maestro-core.tar.gz" > "$ARCHIVE/SHA256SUMS"
```

- [x] **Step 3: Prove the archive restores**

```bash
check=$(mktemp -d)
tar --extract --gzip --file "$ARCHIVE/maestro-core.tar.gz" -C "$check"
diff -r --exclude=target --exclude=.tools maestro-core "$check/maestro-core" \
  && echo "archive verified"
rm -rf "$check"
```

Expected: `archive verified`. Anything else: stop; the reset does not start.

- [x] **Step 4: Record the fixture and test baseline**

```bash
cd maestro-core/crates/document-canonicalization
find examples -type f -print0 | sort -z | xargs -0 sha256sum > "$ARCHIVE/fixtures.sha256"
cd ../..
cargo test --workspace --locked 2>&1 | grep -E '^test ' | sort > "$ARCHIVE/tests-before.txt"
grep -c ' ok$' "$ARCHIVE/tests-before.txt"
grep -c '\.\.\. ignored' "$ARCHIVE/tests-before.txt"
```

Expected: `95` and `4`.

### T002 Rename the crate and remove what does not carry over [US1, US2]

**Files:** move `crates/document-canonicalization/` to `CRATE/`; delete
`docs/superpowers/`, `docs/providers/`, `scripts/bootstrap.sh`,
`.github/workflows/ci.yml`, `CRATE/{Cargo.lock,deny.toml,rust-toolchain.toml,LICENSE,.gitignore,justfile}`,
`CRATE/scripts/`; modify `Cargo.toml`, `Cargo.lock`, `CRATE/Cargo.toml`, the
crate's sources, tests and documents.

- [x] **Step 1: Rename the crate to the workspace's `maestro-<area>` names.**
  Identities do not move: they come from fixed version strings, never from the
  crate's name.

```bash
cd ~/workspace/Orchestration-Maestro/maestro-core
mv crates/document-canonicalization crates/maestro-canonicalization
grep -rlE 'document[-_]canonicalization' Cargo.toml crates/maestro-canonicalization \
  | xargs sed -i -E 's/document-canonicalization/maestro-canonicalization/g;
      s/document_canonicalization/maestro_canonicalization/g'
cargo update --workspace
diff <(tar -xzOf "$ARCHIVE/maestro-core.tar.gz" maestro-core/Cargo.lock | sort) <(sort Cargo.lock)
```

Expected: the sorted `diff` shows only the package's `name` line. Unsorted,
the package's block moves, since `Cargo.lock` orders packages by name. The binary is now
`maestro-canonicalization`; its usage line and the test that runs it follow.

- [x] **Step 2: Stage the private collection's sources in the archive**

```bash
cd ~/workspace/Orchestration-Maestro/maestro-core
STAGING="$ARCHIVE/ctm-collection"
mkdir -p "$STAGING/sources"
grep -rlisE 'bmc|coveo|okta|salesforce|epd' docs/superpowers \
  | xargs -r cp --parents -t "$STAGING/sources"
cp "$CRATE/scripts/verify_corpus.py" "$STAGING/"
ls -R "$STAGING" | head -30
```

Expected: the ingestion audit, the sources proposal and the spider spike among
the listed files, and `verify_corpus.py`.

- [x] **Step 3: Delete**

```bash
rm -rf docs/superpowers docs/providers scripts/bootstrap.sh .github/workflows/ci.yml \
  target "$CRATE/target" "$CRATE/scripts"
rm -f "$CRATE"/{Cargo.lock,deny.toml,rust-toolchain.toml,LICENSE,.gitignore,justfile}
```

- [x] **Step 4: Remove the corpus-sampling instructions.** In `CRATE/README.md`
  and `CRATE/ACCEPTANCE.md`, delete the fenced block running
  `scripts/verify_corpus.py` and the sentence introducing it; in their place
  write: `Corpus sampling runs in the private collection repository, next to the
  corpus it reads.` Verify:

```bash
grep -rn 'verify_corpus\|qualify_tokenizer' "$CRATE"/*.md | grep -v TOKENIZER.md
```

Expected: no output (TOKENIZER.md is rewritten in T015). `DEDUPLICATION.md`
links to a design document that stays in the archive
(`../docs/superpowers/specs/2026-09-21-scoped-exact-dedup-design.md#equality-contract`):
remove that link and keep the sentence, which states the contract itself.

- [x] **Step 5: The tests still pass**

Run: `cargo test --workspace --locked 2>&1 | grep -E '^test result' | awk '{p+=$4} END {print p}'`
Expected: `95`.

- [x] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: keep only the canonicalization crate"
```

### T003 Toolbelt, justfile and hooks [US1]

**Files:** create `mise.toml`, `mise.lock`, `scripts/bootstrap.sh`, `justfile`,
`.github/zizmor.yml`; modify `.pre-commit-config.yaml`, `.gitignore`.

- [x] **Step 1: Copy the provisioning from rust-workflows at `8a55c53`**

```bash
git -C "$RW" rev-parse --short HEAD   # expect 8a55c53
cp "$RW/scripts/bootstrap.sh" scripts/bootstrap.sh
cp "$RW/mise.toml" mise.toml
cp "$RW/mise.lock" mise.lock
```

- [x] **Step 2: Keep only the tools this repository runs.** In `mise.toml`, keep
  the header comment, `[settings]`, and for each of these tools its `# tool:`
  line, its `[tool_alias]` entry and its `[tools]` entry: `mise`, `just`,
  `actionlint`, `zizmor`, `gitleaks`, `shellcheck`, `yamlfmt`, `taplo`, `jaq`,
  `prek`, `cargo-deny`, `cargo-llvm-cov`, `cargo-mutants`, `cargo-machete`,
  `typos`. Delete every other tool's three lines. Then relock and check that no
  kept tool's checksum moved:

```bash
export PATH="$RW/.tools/bin:$PATH"   # rust-workflows' mise and jaq until Step 6
mise trust mise.toml
mise lock --platform linux-x64,linux-x64-musl
diff <(jaq -r --from toml '.tools | keys[]' mise.lock) \
     <(jaq -r --from toml '.tools | keys[]' mise.toml | sort) && echo "same tools"
for tool in $(jaq -r --from toml '.tools | keys[]' mise.toml); do
  a=$(jaq -r --from toml --arg t "$tool" '.tools[$t][0]["platforms.linux-x64"].checksum' mise.lock)
  b=$(jaq -r --from toml --arg t "$tool" '.tools[$t][0]["platforms.linux-x64"].checksum' "$RW/mise.lock")
  [[ "$a" == "$b" ]] || echo "CHECKSUM MOVED: $tool"
done
```

Expected: `same tools` and no `CHECKSUM MOVED` line.

- [x] **Step 3: Write `justfile`**

```just
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
```

- [x] **Step 4: Write `.github/zizmor.yml`**

```yaml
# The workflow security audit `just check` and the commit hook run: zizmor, in
# its pedantic persona, offline. Every finding blocks.
rules:
  unpinned-uses:
    # Every action pinned to a commit, GitHub's own included.
    config:
      policies:
        "*": hash-pin
```

- [x] **Step 5: Point the hook at that configuration and ignore mutation output.**
  In `.pre-commit-config.yaml`, change the zizmor entry to
  `zizmor --offline --persona=pedantic --no-progress --config .github/zizmor.yml`.
  Append `/mutants.out*/` to `.gitignore`.

- [x] **Step 6: Provision and list**

```bash
scripts/bootstrap.sh
just
```

Expected: the recipes `help`, `setup`, `check`, `docs`, `native`, `mutants`.
`just check` still fails here; T004–T021 make it pass.

- [x] **Step 7: Commit**

```bash
git add -A
git commit -m "build: pin the toolbelt and the local gate"
```

---

## Phase 2: User Story 1 — a clean repository under the gates

### T004 Inherit the workspace lints [US1]

**Files:** modify `Cargo.toml`, `CRATE/Cargo.toml`, `CRATE/tests/*.rs` (8 files).

- [x] **Step 1: Workspace members by glob.** In `Cargo.toml` set
  `members = ["crates/*"]`.

- [x] **Step 2: The crate inherits.** Replace `CRATE/Cargo.toml`'s `[package]`
  and lint tables with:

```toml
[package]
name = "maestro-canonicalization"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[lints]
workspace = true
```

  Keep `[dependencies]` as it is.

- [x] **Step 3: Integration tests are test crates.** At the top of each file in
  `CRATE/tests/` put a one-sentence `//!` summary of what the file proves, then
  `#![cfg(test)]`, which lets Clippy's test allowances reach helper functions.
  Example for `tests/cli_contract.rs`:

```rust
//! The command-line tool's contract: arguments, exit codes and saved documents.
#![cfg(test)]
```

  `tests/chunk_native.rs` keeps its existing `//!` line and gains
  `#![cfg(test)]`.

- [x] **Step 4: The suite compiles and passes**

Run: `cargo test --workspace --locked 2>&1 | grep -E '^test result' | awk '{p+=$4; i+=$8} END {print p, i}'`
Expected: `95 4`.

- [x] **Step 5: Record the findings left**

```bash
cargo clippy --workspace --all-targets --locked --keep-going --message-format=short \
  -- --cap-lints warn 2>&1 | grep -cE '^crates/.*: warning:'
```

Expected: `232` (the 244 measured, less the test crates' 11 `unwrap` findings
and their missing crate doc; measured on a copy on 2026-09-24).

- [x] **Step 6: Commit**

```bash
git add -A
git commit -m "build: inherit the workspace lints in the crate"
```

### T005 The repository policies as failing tests [US1, US2]

**Files:** create `crates/maestro-conventions/Cargo.toml`,
`crates/maestro-conventions/src/lib.rs`, `crates/maestro-conventions/tests/policies.rs`.

**Interfaces:**
- Produces: `root() -> PathBuf`, `repository_files(root: &Path) -> io::Result<Vec<PathBuf>>`,
  `counted_lines(text: &str) -> usize`, `names_a_personal_directory(text: &str) -> bool`,
  `broken_links(root: &Path, file: &Path) -> io::Result<Vec<String>>`.

- [x] **Step 1: Write the manifest**

```toml
[package]
name = "maestro-conventions"
description = "Tests that hold the maestro-core repository to its own policies"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[lints]
workspace = true

[dependencies]
pulldown-cmark = { version = "=0.13.4", default-features = false }
```

- [x] **Step 2: Write the helpers' unit tests first.** `src/lib.rs` starts as the
  crate doc and this test module only:

```rust
//! Helpers for the repository's policy tests: the files the repository holds,
//! counted lines, personal directories and Markdown links. The policies live
//! in `tests/policies.rs`, so CI enforces them.

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory under the system's temporary directory.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("policy-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn counted_lines_skip_blank_lines_and_line_comments() {
        assert_eq!(counted_lines("fn a() {}\n\n  // note\n/// doc\n  let b = 1;\n"), 2);
    }

    #[test]
    fn personal_directories_need_a_name_and_a_slash() {
        let home = ["/", "home", "/"].concat();
        let users = ["/", "Users", "/"].concat();
        assert!(names_a_personal_directory(&format!("x {home}alice/notes")));
        assert!(names_a_personal_directory(&format!("{users}bob/")));
        let windows = ["C:", "\\", "Users", "\\"].concat();
        assert!(names_a_personal_directory(&format!("{windows}carol\\notes")));
        assert!(!names_a_personal_directory(&format!("{home}<name>/ ~/workspace /usr/bin")));
        assert!(!names_a_personal_directory(&home));
        assert!(!names_a_personal_directory(&windows));
    }

    #[test]
    fn headings_get_github_anchors_with_numbered_duplicates() {
        let found = anchors("# 1.3 Check, compile\n## `knowledge_search` tool\n## Notes\n## Notes\n");
        let expected = ["13-check-compile", "knowledge_search-tool", "notes", "notes-1"];
        assert_eq!(found, expected.into_iter().map(String::from).collect::<BTreeSet<_>>());
    }

    #[test]
    fn link_targets_include_images_and_skip_code() {
        let text = "[a](b.md#c) ![i](img.svg)\n\n```\n[x](y.md)\n```\n\n`[z](w.md)`\n";
        assert_eq!(link_targets(text), ["b.md#c", "img.svg"]);
    }

    #[test]
    fn broken_links_name_missing_files_and_anchors() {
        let root = scratch("links");
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("docs/b.md"), "# Present\n").unwrap();
        let links = "[1](b.md#present) [2](b.md#absent) [3](c.md) [4](#here) [5](#gone) \
                     [6](https://example.org/x) [7](b.md)";
        fs::write(root.join("docs/a.md"), format!("# Here\n\n{links}\n")).unwrap();
        let problems = broken_links(&root, Path::new("docs/a.md")).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(problems, ["docs/a.md: b.md#absent", "docs/a.md: c.md", "docs/a.md: #gone"]);
    }

    #[test]
    fn repository_files_skip_git_tools_and_build_output() {
        let root = scratch("files");
        for dir in [".git", ".tools/bin", "target/debug", "crates/a/target", "crates/a/src"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        for file in [".git/HEAD", ".tools/bin/x", "target/debug/y", "crates/a/target/z",
                     "crates/a/src/lib.rs", "README.md"] {
            fs::write(root.join(file), "").unwrap();
        }
        let files = repository_files(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(files, [PathBuf::from("README.md"), PathBuf::from("crates/a/src/lib.rs")]);
    }
}
```

- [x] **Step 3: Run them to see them fail**

Run: `cargo test -p maestro-conventions --lib 2>&1 | tail -3`
Expected: compile errors, `cannot find function counted_lines` and the others.

- [x] **Step 4: Implement the helpers.** Insert above the test module:

```rust
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

/// Directories that hold history, downloaded tools or build output, never
/// repository content.
const SKIPPED: [&str; 3] = [".git", ".tools", "target"];

/// The repository root, two levels above this crate.
#[must_use]
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every file under `root` outside [`SKIPPED`] directories, relative and
/// sorted. A walk rather than `git ls-files`: mutation testing runs in a copy
/// without `.git`.
///
/// # Errors
/// Any unreadable directory: a policy that cannot see a file must fail.
pub fn repository_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() && !SKIPPED.iter().any(|name| entry.file_name() == *name) {
                pending.push(entry.path());
            } else if kind.is_file() {
                let path = entry.path();
                files.push(path.strip_prefix(root).map_err(io::Error::other)?.to_path_buf());
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Lines that are neither blank nor `//` comments: the measure of a file's size.
#[must_use]
pub fn counted_lines(text: &str) -> usize {
    text.lines()
        .map(str::trim_start)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .count()
}

/// Whether the text names a person's home directory: a name, then a
/// separator, under `/home/`, `/Users/` or a Windows drive's `Users` folder.
#[must_use]
pub fn names_a_personal_directory(text: &str) -> bool {
    let homes = [
        (["/", "home", "/"].concat(), '/'),
        (["/", "Users", "/"].concat(), '/'),
        (["C:", "\\", "Users", "\\"].concat(), '\\'),
    ];
    homes.iter().any(|(prefix, separator)| {
        text.match_indices(prefix.as_str()).any(|(at, _)| {
            let rest = &text[at + prefix.len()..];
            let name = rest
                .find(|c: char| !(c.is_alphanumeric() || matches!(c, '.' | '_' | '-')))
                .unwrap_or(rest.len());
            name > 0 && rest[name..].starts_with(*separator)
        })
    })
}

/// Every relative link or image in `file` whose target file or heading anchor
/// does not exist, as `file: target` lines.
///
/// # Errors
/// An unreadable Markdown file.
pub fn broken_links(root: &Path, file: &Path) -> io::Result<Vec<String>> {
    let text = fs::read_to_string(root.join(file))?;
    let mut problems = Vec::new();
    for target in link_targets(&text) {
        if target.contains("://") || target.starts_with("mailto:") {
            continue;
        }
        let (path, anchor) = target.split_once('#').unwrap_or((target.as_str(), ""));
        let linked = if path.is_empty() {
            root.join(file)
        } else {
            root.join(file.parent().unwrap_or(Path::new(""))).join(path)
        };
        let markdown = linked.extension().is_some_and(|extension| extension == "md");
        let found = if anchor.is_empty() || !markdown {
            linked.exists()
        } else {
            fs::read_to_string(&linked).is_ok_and(|text| anchors(&text).contains(anchor))
        };
        if !found {
            problems.push(format!("{}: {target}", file.display()));
        }
    }
    Ok(problems)
}

/// The destinations of the links and images in a Markdown text.
fn link_targets(text: &str) -> Vec<String> {
    Parser::new(text)
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                Some(dest_url.to_string())
            }
            _ => None,
        })
        .collect()
}

/// The anchors GitHub gives a Markdown text's headings, duplicates numbered.
fn anchors(text: &str) -> BTreeSet<String> {
    let mut anchors = BTreeSet::new();
    let mut heading: Option<String> = None;
    for event in Parser::new(text) {
        match event {
            Event::Start(Tag::Heading { .. }) => heading = Some(String::new()),
            Event::Text(part) | Event::Code(part) => {
                if let Some(title) = heading.as_mut() {
                    title.push_str(&part);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(title) = heading.take() {
                    let base = slug(&title);
                    let (mut anchor, mut number) = (base.clone(), 0);
                    while !anchors.insert(anchor.clone()) {
                        number += 1;
                        anchor = format!("{base}-{number}");
                    }
                }
            }
            _ => {}
        }
    }
    anchors
}

/// GitHub's heading slug: lower case; letters, digits, `_` and `-` kept;
/// spaces as `-`; everything else dropped.
fn slug(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '_' || c == '-' => Some(c),
            _ => None,
        })
        .collect()
}
```

- [x] **Step 5: The helpers pass**

Run: `cargo test -p maestro-conventions --lib 2>&1 | grep 'test result'`
Expected: `test result: ok. 6 passed`.

- [x] **Step 6: Write the policies**

```rust
//! The repository's policies, checked on every pull request by `cargo test`.
#![cfg(test)]
use maestro_conventions::{
    broken_links, counted_lines, names_a_personal_directory, repository_files, root,
};
use std::{fs, path::PathBuf};

/// Every repository file that reads as UTF-8 text, with its contents.
fn text_files() -> Vec<(PathBuf, String)> {
    let root = root();
    repository_files(&root)
        .unwrap()
        .into_iter()
        .filter_map(|file| {
            let text = fs::read_to_string(root.join(&file)).ok()?;
            Some((file, text))
        })
        .collect()
}

#[test]
fn rust_files_stay_within_500_counted_lines() {
    let oversized: Vec<String> = text_files()
        .into_iter()
        .filter(|(file, _)| file.extension().is_some_and(|extension| extension == "rs"))
        .filter_map(|(file, text)| {
            let lines = counted_lines(&text);
            (lines > 500).then(|| format!("{}: {lines}", file.display()))
        })
        .collect();
    assert!(oversized.is_empty(), "split these files: {oversized:?}");
}

#[test]
fn no_personal_path_in_repository_text() {
    let offenders: Vec<PathBuf> = text_files()
        .into_iter()
        .filter(|(_, text)| names_a_personal_directory(text))
        .map(|(file, _)| file)
        .collect();
    assert!(offenders.is_empty(), "personal paths in {offenders:?}");
}

#[test]
fn no_private_registry_or_quality_service_setting() {
    // Split so that this file does not name them. Documents may say what was
    // dropped; configuration and code may not carry it.
    let settings = [["arti", "factory"].concat(), ["jf", "rog"].concat(), ["son", "ar"].concat()];
    let offenders: Vec<PathBuf> = text_files()
        .into_iter()
        .filter(|(file, _)| file.extension().is_none_or(|extension| extension != "md"))
        .filter(|(_, text)| {
            let text = text.to_lowercase();
            settings.iter().any(|setting| text.contains(setting.as_str()))
        })
        .map(|(file, _)| file)
        .collect();
    assert!(offenders.is_empty(), "registry or quality-service settings in {offenders:?}");
}

#[test]
fn every_member_inherits_the_workspace_lints() {
    for member in fs::read_dir(root().join("crates")).unwrap() {
        let manifest = member.unwrap().path().join("Cargo.toml");
        let text = fs::read_to_string(&manifest).unwrap();
        let name = manifest.display();
        assert!(text.contains("[lints]\nworkspace = true"), "{name} must inherit the lints");
        assert!(!text.contains("[lints."), "{name} must not define its own lints");
    }
}

#[test]
fn every_relative_link_and_anchor_resolves() {
    let root = root();
    let broken: Vec<String> = repository_files(&root)
        .unwrap()
        .into_iter()
        .filter(|file| file.extension().is_some_and(|extension| extension == "md"))
        // Markdown under examples/ is test input, kept byte for byte.
        .filter(|file| !file.components().any(|part| part.as_os_str() == "examples"))
        .flat_map(|file| broken_links(&root, &file).unwrap())
        .collect();
    assert!(broken.is_empty(), "broken links: {broken:?}");
}
```

- [x] **Step 7: See the policies fail for the known reasons**

Run: `cargo test -p maestro-conventions --test policies 2>&1 | grep -E '^test |split these|personal paths'`
Expected: `rust_files_stay_within_500_counted_lines` fails naming `chunk_split.rs`,
`chunks.rs`, `chunk_mapping.rs`, `validate.rs` and `tokenizer.rs` (502 once T002
wrapped its long lines); `no_personal_path_in_repository_text`
fails naming `tokenizer-contract.json` and `TOKENIZER.md`; the other three pass.
A different failure is a finding to fix now.

- [x] **Step 8: Commit**

```bash
git add -A
git commit -m "test: hold the repository to its policies"
```

### T006 Split `chunk_split.rs` into modules [US1]

**Files:** replace `CRATE/src/chunk_split.rs` with
`CRATE/src/chunk_split/{mod,layout,context,prepare,tests}.rs`.

- [x] **Step 1: Move, do not edit.** Following plan D2: `mod.rs` keeps the
  constants, the types and `structure_error`, `layout`, `validate_preparation`,
  `build_drafts`, `combine`; `layout.rs` takes the `impl Layout` methods from
  `owner` to `owned_units`; `context.rs` from `context_entries` to `item_prefix`;
  `prepare.rs` from `body_parts` to `fit_prefix`; `tests.rs` the test module
  with `structural_chunks`. Each file opens with a `//!` line. `mod.rs` declares
  `mod context; mod layout; mod prepare; #[cfg(test)] mod tests;`. A method one
  file calls in another becomes `pub(super)`; nothing else changes.

- [x] **Step 2: Verify behaviour and size**

```bash
cargo test -p maestro-canonicalization --locked 2>&1 | grep -E '^test result' | awk '{p+=$4} END {print p}'
(cd "$CRATE" && sha256sum --check --quiet "$ARCHIVE/fixtures.sha256") && echo "fixtures ok"
cargo test -p maestro-conventions --test policies rust_files 2>&1 | grep -o 'split these files.*'
```

Expected: `95`, `fixtures ok`, and `chunk_split` no longer named.

- [x] **Step 3: Commit:** `refactor: split chunk splitting into modules`.

### T007 Split `chunks.rs` into modules [US1]

**Files:** replace `CRATE/src/chunks.rs` with `CRATE/src/chunks/{mod,identity,validation}.rs`
and `CRATE/src/chunks/tests/{mod,replay}.rs`.

- [x] **Step 1: Move, do not edit.** `mod.rs`: the public types (re-exported by
  `lib.rs` as before through `pub use chunks::*`), constants, `chunk_documents`,
  `chunk_with_count`, `build_batch`, `record_bytes`, `invalid_chunks`;
  `identity.rs`: `insert_prepared_group` and `PreparedGroups`; `validation.rs`:
  `validate_coverage`, `validate_chunks`; `tests/mod.rs`: the test module, whose
  536 counted lines exceed 500 on their own, so its two replay tests go to
  `tests/replay.rs`. `pub(super)` where a moved item is called from its parent.

- [x] **Step 2: Verify:** the same three commands as T006 Step 2; expected `95`,
  `fixtures ok`, `chunks.rs` no longer named.

- [x] **Step 3: Commit:** `refactor: split chunk assembly into modules`.

### T008 Split `chunk_mapping.rs` into modules [US1]

**Files:** replace `CRATE/src/chunk_mapping.rs` with `CRATE/src/chunk_mapping/{mod,slice,tests}.rs`.

- [x] **Step 1: Move, do not edit.** `mod.rs`: `invalid_mapping`, `map_document`
  and the mapper (`walk` through `new_unit`); `slice.rs`: `mapped_slice`,
  `map_accounting` (both `pub(crate)` as today, re-exported from `mod.rs` so
  `crate::chunk_mapping::mapped_slice` keeps resolving); `tests.rs`: the test
  module.

- [x] **Step 2: Verify:** as T006 Step 2; `chunk_mapping.rs` no longer named.

- [x] **Step 3: Commit:** `refactor: split source mapping into modules`.

### T009 Split `validate.rs` into modules [US1]

**Files:** replace `CRATE/src/validate.rs` with `CRATE/src/validate/{mod,blocks,sections}.rs`.

- [x] **Step 1: Move, do not edit.** `mod.rs`: `validate_document`, `replay`,
  `validate_structure`, `check_gap`, `contains`, `issue`; `blocks.rs`:
  `validate_block`, `validate_container_gaps`, `validate_inline`,
  `validate_table`, `table_gap`; `sections.rs`: `validate_sections`,
  `validate_extractor`. `crate::validate::contains` keeps resolving (ten callers).

- [x] **Step 2: Verify:** as T006 Step 2; the policy test now passes except for
  `tokenizer.rs` (T010) and the personal paths (T015).

- [x] **Step 3: Commit:** `refactor: split document validation into modules`.

### T010 Split `tokenizer.rs` before it grows [US1, US2]

**Files:** replace `CRATE/src/tokenizer.rs` with `CRATE/src/tokenizer/{mod,process,tests}.rs`.

- [x] **Step 1: Move, do not edit.** `process.rs`: `MAX_STDOUT_BYTES`,
  `MAX_STDERR_BYTES`, `read_bounded`, `Reap`, `run_native` (`pub(super)`);
  `mod.rs`: everything else except the test module, which goes to `tests.rs`.
  Both `include_str!("../tokenizer-contract.json")` become
  `include_str!("../../tokenizer-contract.json")`. `tests.rs` adds
  `use super::process::run_native;`.

- [x] **Step 2: Verify:** as T006 Step 2.

- [x] **Step 3: Commit:** `refactor: split the native tokenizer into modules`.

### T011 Split the five production functions over the limits [US1]

**Files:** modify `CRATE/src/chunk_split/context.rs`, `CRATE/src/chunks/{mod,identity,validation}.rs`,
`CRATE/src/validate/{mod,blocks}.rs`, `CRATE/src/dedup.rs`.

Every extraction moves code verbatim into a named function and replaces it with
a call; control flow and error values stay the same.

- [x] **Step 1: `Layout::context_entries`** (181 lines, complexity 21). Extract,
  in `context.rs`:
  - `fn heading_chain(&self, first: usize) -> Result<Vec<String>, Error>`: the
    `while let Some(id) = section` loop and the `reverse`;
  - `fn heading_entries(&self, body: &Body, headings: &[String]) -> Vec<ContextEntry>`:
    the loop over `headings`;
  - `fn ancestor_entries(&self, body: &Body) -> Result<Vec<ContextEntry>, Error>`:
    the loop over `body.fragments`, owning `seen`; its per-ancestor work goes to
    `fn ancestor_block_entries(&self, block: &Block, depth: usize, own_item: Option<&str>, seen: &mut BTreeSet<String>) -> Result<Vec<ContextEntry>, Error>`,
    which calls `fn list_item_units(&self, item_id: &str) -> Vec<usize>`,
    `fn task_marker_units(&self, item_id: &str) -> Result<Vec<usize>, Error>` and
    `fn definition_term(&self, description: &Block) -> Result<&Block, Error>`
    (lifetime as the `blocks` map gives it);
  - `fn table_header_entry(&self, body: &Body, first: usize) -> Result<Option<ContextEntry>, Error>`.
  What remains is the four calls, the `retain` and the `sort_by_key`.

- [x] **Step 2: `build_batch`** (102 lines). In `identity.rs`:

```rust
/// The identities of one prepared input, the grouping key of identical inputs.
pub(super) struct PreparedIdentity {
    /// `sha256:` of the profile, the counter's contract and the prepared input.
    pub(super) fingerprint: String,
    /// The serialized record the fingerprint digests.
    pub(super) bytes: Vec<u8>,
    /// The scoped group identifier derived from the fingerprint.
    pub(super) group_id: String,
}
```

  and move the two identity computations into
  `pub(super) fn chunk_id(deduplication: &Deduplication<'_>, document: &CanonicalDocument, mapped: &MappedDocument, content: &ChunkContent, tokenizer_contract_id: &str) -> Result<String, Error>`
  and
  `pub(super) fn prepared_identity(deduplication: &Deduplication<'_>, tokenizer_contract_id: &str, prepared_input: &str) -> Result<PreparedIdentity, Error>`.

- [x] **Step 3: `validate_chunks`** (112 lines). In `validation.rs`, extract
  `fn check_separator(part: &InputPart) -> Result<(), Error>` (the
  `FormattingSeparator` branch), `fn replay_part(part: &InputPart, mapped: &MappedDocument, markdown: &str) -> Result<Vec<Contribution>, Error>`
  (the contribution loop and the text and mapping comparison; returns the
  part's primary contributions) and
  `fn check_fragment_order(chunk: &ChunkContent, primary: &[Contribution]) -> Result<(), Error>`
  (the cursor walk over `chunk.fragments`).

- [x] **Step 4: `validate_structure`** (117 lines). In `validate/mod.rs`, extract
  `check_reference(doc, markdown, issues)`, `check_source_ledger(doc, markdown, by_id, issues)`,
  `check_usable(doc, issues)`, `check_artifacts(markdown, issues)` and
  `check_uncovered(doc, markdown, issues)`, each with the parameter types the
  moved code already uses and `issues: &mut Vec<Finding>` last.

- [x] **Step 5: `validate_block`** (123 lines). In `validate/blocks.rs`, extract
  `check_children(block, markdown, by_id, issues)` (the loop over
  `structured_content.children`), `check_parent_reference(block, by_id, issues)`,
  `check_assets(block, issues)` and `check_attributes(block, by_id, markdown, issues)`
  (the final `match`).

- [x] **Step 6: `insert_group`** (six parameters). Its map key becomes one
  parameter:

```rust
fn insert_group(
    groups: &mut Groups,
    scope: &DedupScope,
    key: (Representation, String),
    bytes: Vec<u8>,
    index: usize,
) -> Result<(), Error> {
    let (representation, hash) = key;
    // the existing body, unchanged
}
```

  and each caller passes `(representation, hash)`.

- [x] **Step 7: Verify**

```bash
cargo clippy --workspace --all-targets --locked --message-format=short -- --cap-lints warn 2>&1 \
  | grep -E 'too many lines|cognitive complexity|too many arguments' | grep '/src/' | grep -v '/tests'
cargo test -p maestro-canonicalization --locked 2>&1 | grep -E '^test result' | awk '{p+=$4} END {print p}'
(cd "$CRATE" && sha256sum --check --quiet "$ARCHIVE/fixtures.sha256") && echo "fixtures ok"
```

Expected: no production line under `/src/` (the unit-test modules are T012's),
`95`, `fixtures ok`.

- [x] **Step 8: Commit:** `refactor: split the functions over the size limits`.

### T012 Split the six test functions over the limits [US1]

**Files:** modify `CRATE/tests/properties.rs`, `CRATE/tests/chunk_native.rs`,
`CRATE/tests/dedup_contract.rs`, `CRATE/tests/cli_contract.rs`,
`CRATE/src/chunk_mapping/tests.rs`, `CRATE/src/chunks/tests.rs`.

- [x] **Step 1: One scenario per function.** In each over-limit test, each block
  that sets up and asserts one scenario becomes a helper named after what it
  proves (for example `fn check_table_rows_keep_their_header(...)`) called
  from the test, or its own `#[test]` when it needs no shared setup. Every
  assertion is kept with its values; none is weakened or dropped.

- [x] **Step 2: Verify**

```bash
cargo clippy --workspace --all-targets --locked --message-format=short -- --cap-lints warn 2>&1 \
  | grep -cE 'too many lines|cognitive complexity|too many arguments'
cargo test -p maestro-canonicalization --locked 2>&1 | grep -E '^test result' | awk '{p+=$4; i+=$8} END {print p, i}'
```

Expected: `0`; at least `95` passed and `4` or more ignored (a split native test
adds ignored tests). List the renamed tests in the commit body.

- [x] **Step 3: Commit:** `test: give each long test one scenario per function`.

### T013 Fix the other pedantic findings [US1]

**Files:** as each finding names.

- [x] **Step 1: Apply the fixes.** The 41 findings measured on 2026-09-24, less
  the 11 `unwrap` findings T004 removed:

| Finding | Where (item) | Fix |
| --- | --- | --- |
| `semicolon_if_nothing_returned` (5) | mapper `walk` (2), `metadata` merge, `validate_block` table arm, a `dedup_contract` test | Add the `;` |
| `needless_pass_by_value` (5) | mapper `context` text, `Layout::nearest` kind, `main` arguments (2), `store` | Borrow, as Clippy's help says |
| `wildcard_imports` (2) | `chunk_mapping`, `chunk_split` | List the imports Clippy names |
| `assigning_clones` (2) | `context_entries` section, `validate::replay` | `clone_from` |
| `unnecessary_wraps` (1) | `chunks` tests' `fake_count` | Return `usize`; wrap at each call: `\|input\| Ok(fake_count(input))` |
| `must_use_candidate` (5) | `content`, `model` (2), `NativeTokenizer::contract_id`, `validate_document` | `#[must_use]` |
| `doc_markdown` (2) | `content` (2) | Backticks |
| `map_unwrap_or` (1) | `lib.rs` `canonicalize` | `map_or_else` |
| `struct_excessive_bools` (1) | `ParserOptions` | `#[expect(clippy::struct_excessive_bools, reason = "each field is an independent, serialized parser switch that identities include")]` |
| `large_stack_arrays` (1) | `verify_artifact` buffer | `vec![0; 64 * 1024]` |
| `nonminimal_bool` (1) | `validate_container_gaps` | Clippy's simplified expression |
| `match_wildcard_for_single_variants` (2) | `validate_table` | `ContentNode::Inline { .. }` |
| `cast_possible_truncation` (2) | `tests/acceptance.rs` | `usize::try_from(value).unwrap()` |

- [x] **Step 2: Verify**

```bash
cargo clippy --workspace --all-targets --locked --message-format=json -- --cap-lints warn 2>/dev/null \
  | jaq -r 'select(.reason == "compiler-message") | .message | select(.level == "warning")
      | select(.code.code != "clippy::missing_docs_in_private_items") | .spans[0].file_name' \
  | wc -l
cargo test -p maestro-canonicalization --locked 2>&1 | grep -E '^test result' | awk '{p+=$4} END {print p}'
```

Expected: `0`, and the passing count of T012. Count from JSON: Cargo replays cached
diagnostics in the format they were first rendered in, so a `short` count can
miss findings compiled during an earlier JSON run.

- [x] **Step 3: Commit:** `refactor: meet the pedantic lints`.

### T014 Document every item [US1]

**Files:** every `CRATE/src/` file with a `missing documentation` finding.

- [x] **Step 1: Write the docs.** For each finding, one `///` line saying what
  the item is or does in this crate's words (CONTEXT.md terms), for example:

```rust
/// Byte range of a unit's text inside the prepared input.
prepared_range: TextRange,
```

  A doc line that only repeats the item's name is not accepted in review.

- [x] **Step 2: Clippy and rustdoc are clean**

```bash
cargo clippy --workspace --all-targets --locked -- -D warnings && echo "clippy clean"
just docs && echo "docs clean"
```

Expected: `clippy clean`, `docs clean`.

- [x] **Step 3: Commit:** `docs: document every item of the crate`.

---

## Phase 3: User Story 2 — nothing private in public repositories

### T015 The native binding and the path-free profile [US2]

**Files:** create `CRATE/src/tokenizer/binding.rs`; modify `CRATE/src/tokenizer/{mod,tests}.rs`,
`CRATE/tokenizer-contract.json`, `CRATE/TOKENIZER.md`, `CRATE/CHUNKING.md`.

**Interfaces:**
- Produces: `NativeBinding { model, counter, library_directory, source_root: PathBuf }`,
  `NativeBinding::from_variable(Option<OsString>) -> Result<Self, Error>`,
  `NativeBinding::from_file(&Path) -> Result<Self, Error>`,
  `NativeBinding::from_environment() -> Result<Self, Error>`, all `pub(crate)`;
  `NativeTokenizer::open()` unchanged.

- [x] **Step 1: Write the failing tests.** In `tokenizer/tests.rs` add:

```rust
use super::binding::NativeBinding;
use std::path::PathBuf;

fn write_binding(scratch: &Scratch, text: &str) -> PathBuf {
    let path = scratch.0.join("binding.json");
    fs::write(&path, text).unwrap();
    path
}

#[test]
fn a_binding_resolves_relative_paths_against_its_own_directory() {
    let scratch = Scratch::new();
    let path = write_binding(
        &scratch,
        r#"{"schema":"maestro-native-binding/1","model":"models/m.gguf",
            "counter":"/somewhere/bin/count","library_directory":"lib","source_root":"src"}"#,
    );
    let binding = NativeBinding::from_file(&path).unwrap();
    assert_eq!(binding.model, scratch.0.join("models/m.gguf"));
    assert_eq!(binding.counter, PathBuf::from("/somewhere/bin/count"));
    assert_eq!(binding.library_directory, scratch.0.join("lib"));
    assert_eq!(binding.source_root, scratch.0.join("src"));
}

#[test]
fn a_binding_refusal_names_the_variable_and_the_schema() {
    let scratch = Scratch::new();
    let fields = r#""model":"m","counter":"c","library_directory":"l","source_root":"s""#;
    let oversized = format!(
        r#"{{"schema":"maestro-native-binding/1",{fields},"x":"{}"}}"#,
        "a".repeat(1024 * 1024)
    );
    for text in [
        format!(r#"{{"schema":"maestro-native-binding/2",{fields}}}"#),
        format!(r#"{{"schema":"maestro-native-binding/1",{fields},"extra":1}}"#),
        r#"{"schema":"maestro-native-binding/1","model":"m"}"#.to_owned(),
        "not json".to_owned(),
        oversized,
    ] {
        let path = write_binding(&scratch, &text);
        let error = NativeBinding::from_file(&path).unwrap_err().to_string();
        assert!(error.contains("MAESTRO_NATIVE_BINDING"), "{error}");
        assert!(error.contains("maestro-native-binding/1"), "{error}");
    }
    let unset = NativeBinding::from_variable(None).unwrap_err().to_string();
    assert!(unset.contains("MAESTRO_NATIVE_BINDING"), "{unset}");
    assert!(NativeBinding::from_file(&scratch.0.join("missing.json")).is_err());
}

#[test]
fn the_committed_profile_names_files_never_machine_paths() {
    let profile = parse_contract(include_str!("../../tokenizer-contract.json")).unwrap();
    assert_eq!(profile["schema"], "local-tokenizer-contract/2");
    assert!(!profile.to_string().contains("\"path\""));
    let libraries = array_at(&profile, "/artifacts/libraries").unwrap();
    let sources = array_at(&profile, "/artifacts/sources").unwrap();
    for record in libraries.iter().chain(sources) {
        let file = text_at(record, "/file").unwrap();
        assert!(!Path::new(file).is_absolute() && !file.contains(".."), "{file}");
    }
}
```

  Change `library_aliases_cannot_redirect_away_from_pinned_files` to pass paths:
  `let libraries = [path.clone()];`. Change
  `invocation_replaces_environment_and_preserves_model_argument` to build the
  command from a binding and to expect the library path:

```rust
let binding = NativeBinding {
    model: "/a path/model;literal.gguf".into(),
    counter: "/usr/bin/python3".into(),
    library_directory: "/somewhere/lib".into(),
    source_root: "/somewhere/src".into(),
};
let mut command = configured_command(&contract, &binding).unwrap();
// The existing `run_native` call and the parse of `result` stay; then:
assert_eq!(
    environment.keys().map(String::as_str).collect::<BTreeSet<_>>(),
    BTreeSet::from(["CUDA_VISIBLE_DEVICES", "LC_ALL", "LD_LIBRARY_PATH", "PATH"])
);
assert_eq!(environment["LD_LIBRARY_PATH"], "/somewhere/lib");
```

- [x] **Step 2: See them fail**

Run: `cargo test -p maestro-canonicalization --lib tokenizer 2>&1 | tail -3`
Expected: compile errors (`NativeBinding` unresolved).

- [x] **Step 3: Write `tokenizer/binding.rs`**

```rust
//! Where this machine keeps the artifacts the tokenizer profile fingerprints.
use super::process::read_bounded;
use crate::Error;
use serde::Deserialize;
use std::{
    ffi::OsString,
    fs::File,
    path::{Path, PathBuf},
};

/// The environment variable that names the binding file.
const BINDING_VARIABLE: &str = "MAESTRO_NATIVE_BINDING";
/// The only binding schema this version reads.
const BINDING_SCHEMA: &str = "maestro-native-binding/1";
/// A binding is a few paths; anything larger is not one.
const MAX_BINDING_BYTES: usize = 1024 * 1024;

/// Local paths of the qualified artifacts, resolved from a binding file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeBinding {
    /// The GGUF model the counter loads.
    pub(crate) model: PathBuf,
    /// The vocabulary-only counter executable.
    pub(crate) counter: PathBuf,
    /// The directory holding the counter's shared libraries.
    pub(crate) library_directory: PathBuf,
    /// The llama.cpp source tree the counter was built from.
    pub(crate) source_root: PathBuf,
}

/// The binding file as written, before its paths are resolved.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingFile {
    /// Must equal [`BINDING_SCHEMA`].
    schema: String,
    /// See [`NativeBinding::model`].
    model: PathBuf,
    /// See [`NativeBinding::counter`].
    counter: PathBuf,
    /// See [`NativeBinding::library_directory`].
    library_directory: PathBuf,
    /// See [`NativeBinding::source_root`].
    source_root: PathBuf,
}

/// Every binding refusal names the variable and the schema, never a path or
/// the file's contents.
fn refusal(problem: &str) -> Error {
    Error(format!(
        "{problem}: {BINDING_VARIABLE} must name a {BINDING_SCHEMA} file (see TOKENIZER.md)"
    ))
}

impl NativeBinding {
    /// Read the binding file that `MAESTRO_NATIVE_BINDING` names.
    pub(crate) fn from_environment() -> Result<Self, Error> {
        Self::from_variable(std::env::var_os(BINDING_VARIABLE))
    }

    /// Read the binding file a variable's value names; an unset variable is refused.
    pub(crate) fn from_variable(value: Option<OsString>) -> Result<Self, Error> {
        let path = value.ok_or_else(|| refusal("no native binding"))?;
        Self::from_file(Path::new(&path))
    }

    /// Read a binding file; a relative path resolves against its directory.
    pub(crate) fn from_file(path: &Path) -> Result<Self, Error> {
        let file = File::open(path).map_err(|_| refusal("unreadable native binding"))?;
        let bytes = read_bounded(file, MAX_BINDING_BYTES)
            .map_err(|_| refusal("native binding unreadable or over 1 MiB"))?;
        let written: BindingFile =
            serde_json::from_slice(&bytes).map_err(|_| refusal("invalid native binding"))?;
        if written.schema != BINDING_SCHEMA {
            return Err(refusal("unsupported native binding schema"));
        }
        let base = path.parent().unwrap_or_else(|| Path::new(""));
        let resolve = |bound: PathBuf| if bound.is_absolute() { bound } else { base.join(bound) };
        Ok(Self {
            model: resolve(written.model),
            counter: resolve(written.counter),
            library_directory: resolve(written.library_directory),
            source_root: resolve(written.source_root),
        })
    }
}
```

  Make `read_bounded` in `process.rs` `pub(super)`.

- [x] **Step 4: Rewrite the profile without paths**

```bash
cd "$CRATE"
jaq '.schema = "local-tokenizer-contract/2"
  | del(.contract_id)
  | .artifacts.model |= del(.path)
  | .artifacts.counter |= del(.path)
  | .artifacts.libraries |= map({file: (.path | split("/") | last), bytes, sha256})
  | .artifacts.sources |= map({file: (.path | split("/llama.cpp/") | last), bytes, sha256})
  | .contract_id = "sha256:3546447555757daa4996a2e2e708bc67bce4389e8cee3f8386ed503eeaa6d01c"' \
  tokenizer-contract.json > profile.json
mv profile.json tokenizer-contract.json
cd ../..
```

- [x] **Step 5: Read the artifacts through the binding.** In `tokenizer/mod.rs`:
  `mod binding;` and `use binding::NativeBinding;`; `CONTRACT_ID` becomes
  `"sha256:3546447555757daa4996a2e2e708bc67bce4389e8cee3f8386ed503eeaa6d01c"`;
  then:

```rust
pub struct NativeTokenizer {
    /// The committed qualification profile.
    contract: Value,
    /// Where this machine keeps the profile's artifacts.
    binding: NativeBinding,
}

impl NativeTokenizer {
    /// Verify the approved profile and its artifacts where
    /// `MAESTRO_NATIVE_BINDING` says they are.
    ///
    /// # Errors
    /// Refuses a missing or invalid binding, unavailable or changed artifacts
    /// and invalid profiles.
    pub fn open() -> Result<Self, Error> {
        let counter = Self {
            contract: parse_contract(include_str!("../../tokenizer-contract.json"))?,
            binding: NativeBinding::from_environment()?,
        };
        counter.verify_artifacts()?;
        Ok(counter)
    }

    // `contract_id` stays as it is.

    pub fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error> {
        verify_record(&self.binding.counter, &self.contract["artifacts"]["counter"])?;
        verify_libraries(&self.library_paths()?)?;
        let mut command = configured_command(&self.contract, &self.binding)?;
        // The `timeout` lookup and the `run_native` call that follow stay as they are.
    }

    pub fn verify_artifacts(&self) -> Result<(), Error> {
        verify_record(&self.binding.model, &self.contract["artifacts"]["model"])?;
        verify_record(&self.binding.counter, &self.contract["artifacts"]["counter"])?;
        for source in array_at(&self.contract, "/artifacts/sources")? {
            verify_record(&self.binding.source_root.join(text_at(source, "/file")?), source)?;
        }
        for library in array_at(&self.contract, "/artifacts/libraries")? {
            let path = self.binding.library_directory.join(text_at(library, "/file")?);
            verify_record(&path, library)?;
        }
        verify_libraries(&self.library_paths()?)
    }

    /// The profile's libraries inside the bound directory.
    fn library_paths(&self) -> Result<Vec<PathBuf>, Error> {
        array_at(&self.contract, "/artifacts/libraries")?
            .iter()
            .map(|library| Ok(self.binding.library_directory.join(text_at(library, "/file")?)))
            .collect()
    }
}

fn configured_command(contract: &Value, binding: &NativeBinding) -> Result<Command, Error> {
    let mut command = Command::new(&binding.counter);
    for argument in array_at(contract, "/invocation/args")? {
        let argument = argument.as_str().ok_or_else(invalid_contract)?;
        if argument == "{model}" {
            command.arg(&binding.model);
        } else {
            command.arg(argument);
        }
    }
    command.env_clear();
    for (name, value) in contract
        .pointer("/invocation/environment_replace")
        .and_then(Value::as_object)
        .ok_or_else(invalid_contract)?
    {
        command.env(name, value.as_str().ok_or_else(invalid_contract)?);
    }
    // The counter's RUNPATH names the directory it was built in; the loader
    // searches LD_LIBRARY_PATH first, so the verified libraries are the loaded ones.
    command.env("LD_LIBRARY_PATH", &binding.library_directory);
    Ok(command)
}

fn verify_record(path: &Path, record: &Value) -> Result<(), Error> {
    verify_artifact(
        path,
        record["bytes"].as_u64().ok_or_else(invalid_contract)?,
        text_at(record, "/sha256")?,
    )
}
```

  and `verify_libraries(libraries: &[PathBuf])` takes the paths directly: its
  loop becomes `for path in libraries {` with `let directory = path.parent()…`
  and the rest unchanged. Keep the existing doc comments on `contract_id`,
  `token_ids` and `verify_artifacts`.

- [x] **Step 6: The unit tests pass**

Run: `cargo test -p maestro-canonicalization --lib tokenizer 2>&1 | grep 'test result'`
Expected: `ok`, with the three new tests among them.

- [x] **Step 7: Bind this machine's artifacts.** The paths come from the archived
  schema-1 profile, so none is typed into the repository:

```bash
old=$(mktemp -d)
tar --extract --gzip --file "$ARCHIVE/maestro-core.tar.gz" -C "$old" \
  maestro-core/crates/document-canonicalization/tokenizer-contract.json
profile1="$old/maestro-core/crates/document-canonicalization/tokenizer-contract.json"
mkdir -p ~/.config/maestro
jaq '{schema: "maestro-native-binding/1",
      model: .artifacts.model.path,
      counter: .artifacts.counter.path,
      library_directory: (.artifacts.libraries[0].path | split("/") | .[:-1] | join("/")),
      source_root: (.artifacts.sources[0].path | split("/llama.cpp/") | .[0] + "/llama.cpp")}' \
  "$profile1" > ~/.config/maestro/native-binding.json
export MAESTRO_NATIVE_BINDING=~/.config/maestro/native-binding.json
```

- [x] **Step 8: The native tests pass through the binding**

Run: `just native 2>&1 | grep 'test result'`
Expected: `test result: ok. 4 passed` (or the count after T012), in about 4 minutes.

- [x] **Step 9: A relocated copy works through a relative binding**

```bash
reloc=$(mktemp -d "$PWD/target/relocated.XXXXXX")
mkdir -p "$reloc/lib" "$reloc/src"
ln "$(jaq -r .model "$MAESTRO_NATIVE_BINDING")" "$reloc/model.gguf"
ln "$(jaq -r .counter "$MAESTRO_NATIVE_BINDING")" "$reloc/llama-tokenize"
cp -al "$(jaq -r .library_directory "$MAESTRO_NATIVE_BINDING")"/*.so* "$reloc/lib/"
(cd "$(jaq -r .source_root "$MAESTRO_NATIVE_BINDING")" \
  && jaq -r '.artifacts.sources[].file' "$OLDPWD/$CRATE/tokenizer-contract.json" \
  | xargs cp --parents -t "$reloc/src")
printf '%s\n' '{"schema": "maestro-native-binding/1", "model": "model.gguf",' \
  '"counter": "llama-tokenize", "library_directory": "lib", "source_root": "src"}' \
  > "$reloc/binding.json"
MAESTRO_NATIVE_BINDING="$reloc/binding.json" just native 2>&1 | grep 'test result'
rm -rf "$reloc"
```

Expected: the same `test result: ok` as Step 8. Identities cannot differ: the
binding never enters them.

- [x] **Step 10: A missing binding is refused, not skipped**

Run: `env -u MAESTRO_NATIVE_BINDING just native 2>&1 | grep -c 'MAESTRO_NATIVE_BINDING must name'`
Expected: at least `1`, and the run fails.

- [x] **Step 11: Rewrite `TOKENIZER.md`** with this content (the qualification
  facts stay; machine paths, old task numbers and the removed script's
  instructions go):

````markdown
# Local GGUF tokenizer

`NativeTokenizer` counts tokens with the upstream llama.cpp `llama-tokenize`
tool against a pinned GGUF vocabulary, in a subprocess, without a model forward
pass. The committed profile pins every artifact by size and SHA-256; a local
binding says where this machine keeps them.

## Qualified identity

| Item | Qualified value |
| --- | --- |
| Model | BGE-M3 Q8_0 GGUF, 634,553,760 bytes, SHA-256 `aa473d51f451a22f0fcf39ba3330c14bed38a385712b1113440f69df4047a173` |
| llama.cpp | Commit `77f132cb1df1de6357617aeaf0ca04c02cf15fb1` (tag `b10681`) and its shared libraries |
| Counter | Unmodified upstream `tools/tokenize/tokenize.cpp`, 40,720 bytes, SHA-256 `7a74998544dfae5ce7f81da0c798feae9b44f05c56170b1577b7236b2909694e` |
| Profile | [tokenizer-contract.json](tokenizer-contract.json), schema `local-tokenizer-contract/2`, identifier `sha256:3546447555757daa4996a2e2e708bc67bce4389e8cee3f8386ed503eeaa6d01c` |

The identifier is the SHA-256 of the profile's JSON with sorted keys, no extra
whitespace and no escaping of non-ASCII text, `contract_id` excluded. Chunk and
prepared-input identities include it; where the artifacts sit does not.

## Bind the artifacts on a machine

Write a binding file outside the repository and name it in
`MAESTRO_NATIVE_BINDING`:

```json
{
  "schema": "maestro-native-binding/1",
  "model": "models/bge-m3-q8_0.gguf",
  "counter": "bin/llama-tokenize",
  "library_directory": "/opt/llama.cpp/build/bin",
  "source_root": "/opt/llama.cpp"
}
```

- A relative path resolves against the binding file's directory.
- The file holds at most 1 MiB and exactly these keys.
- Every artifact must match the profile's size and SHA-256; symbolic links
  to artifacts are refused.
- The library directory holds exactly the profile's libraries and their
  version aliases, and the counter runs with `LD_LIBRARY_PATH` set to it.

Then run the native tests: `just native`. Without a binding they fail with an
error naming `MAESTRO_NATIVE_BINDING`; they are never skipped silently.

## Exact input and counting contract

1. **Prepare one string:** `ordered-input-parts/v1` concatenates the ordered
   `input_parts[].text` verbatim. Titles, headings, repeated table headers,
   list context and separators are explicit parts. No hidden prefix or suffix,
   query instruction, trimming, escaping, line-ending conversion or caller
   Unicode normalization is applied.
2. **Tokenize the complete string:** its UTF-8 bytes go to stdin with
   `--offline --stdin --no-escape --ids`. The GGUF's own normalization applies
   inside llama.cpp; it never rewrites the retained input.
3. **Both special-token controls on:** `add_special=true`, `parse_special=true`;
   the model adds BOS `0` and EOS `2`. Special-looking literals follow this
   GGUF's vocabulary.
4. **Count without padding or truncation:** target 500, hard maximum 700,
   context and separators included. Above 700, chunking splits; nothing is
   clipped.
5. **Local execution boundary:** the environment is replaced by the profile's,
   GPU visibility cleared, library fingerprints checked, a 30-second timeout
   enforced, and only a JSON array of valid IDs accepted.

The GGUF declares a `t5` tokenizer, which this llama.cpp maps to its unigram
tokenizer: 250,002 entries and an embedded normalization map, all pinned by
the model's hash.

## Why this matches the embedding path

In the pinned llama.cpp source, `tools/server/server-context.cpp`
(`handle_embeddings_impl`) tokenizes string inputs with `add_special=true`,
`parse_special=true`; `tools/server/server-common.cpp` delegates to
`common_tokenize` without an instruction or chat template; and
`tools/tokenize/tokenize.cpp` reads stdin verbatim with `--no-escape` and calls
the same tokenizer. No live router call was made; S1 checks the serving path
against this profile before indexing (ADR-0008).

## Qualification results

41 synthetic complete inputs, each tokenized twice, compared by ordered IDs:

| Check | Result |
| --- | --- |
| Multilingual, Unicode, code, headings, tables, whitespace, NUL, literal specials, inert instructions | Deterministic; input hashes and IDs retained |
| Boundaries | Exact 499/500/501 and 699/700/701 counts; 8,192/8,193 counted in full |
| Specials | `Hello world` gives `[0,35378,8999,2]`; without automatic specials `[35378,8999]` |
| Cached ONNX tokenizer as a substitute | 19/41 matched, 22/41 differed: rejected |

| Input | GGUF count | Cached tokenizer | Difference |
| --- | ---: | ---: | --- |
| `"  a   b  "` | 4 | 5 | An extra whitespace ID `6` before EOS |
| Whitespace only | 2 | 3 | An extra whitespace ID |
| `"left   <mask>   right"` | 8 | 5 | Mask ID `250001` where this GGUF tokenizes the literal |
| 700-token cases | 700 | 701 | Trailing whitespace |

The comparison tokenizer both overcounts and undercounts, so it is no
conservative bound. The Python qualification script that produced these
results is kept with the S0 archive; S1 replaces it with Rust parity tests.

## Build the counter

With `LLAMA_CPP` naming a llama.cpp checkout at the pinned commit and
`LLAMA_LIB` its built library directory:

```bash
mkdir -p target/tokenizer-qualification
c++ -std=c++17 -O3 -DNDEBUG \
  -DGGML_BACKEND_SHARED -DGGML_SHARED -DGGML_USE_CPU -DGGML_USE_CUDA -DLLAMA_SHARED \
  -I"$LLAMA_CPP/common" -I"$LLAMA_CPP/vendor" -I"$LLAMA_CPP/include" \
  -I"$LLAMA_CPP/ggml/include" "$LLAMA_CPP/tools/tokenize/tokenize.cpp" \
  -L"$LLAMA_LIB" -Wl,-rpath,"$LLAMA_LIB" \
  "$LLAMA_LIB/libllama-common.so.0.3.0" "$LLAMA_LIB/libllama.so.0.3.0" \
  "$(readlink -f "$LLAMA_LIB/libggml.so")" -o target/tokenizer-qualification/llama-tokenize
```

A rebuilt counter has new bytes: it needs a new profile and review, not a
silent swap. The tool supports `--no-parse-special` but not `--parse-special`
(true is its default); `--no-bos` also drops EOS and is a negative control only.

## Limitations

The subprocess reloads the vocabulary for each input; a persistent adapter
waits for a measured need. CUDA-linked libraries stay dependencies with GPU
visibility cleared. This is not a universal tokenizer equivalence, an
authenticity certification of the model's origin, or a claim that embedding
inference accepts 8,193 tokens.
````

- [x] **Step 12: Update `CHUNKING.md`.** Replace
  `exist at the local paths pinned by [tokenizer-contract.json](tokenizer-contract.json)`
  with `exist where the local binding says (see [TOKENIZER.md](TOKENIZER.md))`,
  and any old contract identifier with the new one:

```bash
grep -rn 'c70d8e1a\|local paths pinned' "$CRATE"/*.md
```

Expected after the edit: no output.

- [x] **Step 13: The personal-path policy passes**

Run: `cargo test -p maestro-conventions --test policies 2>&1 | grep 'test result'`
Expected: `test result: ok. 5 passed`.

- [x] **Step 14: Commit:** `feat: read the tokenizer's artifacts through a local binding`.

---

## Phase 4: User Story 4 — the process is in place

### T016 Spec Kit scaffolding [US4]

**Files:** create `.specify/templates/`, `.specify/scripts/bash/`,
`.specify/init-options.json`, `.github/skills/speckit-*/` (Spec Kit 1.0.1 writes Copilot
skills there); keep `.specify/memory/constitution.md`.

- [x] **Step 1: Scaffold aside and copy only what is missing**

```bash
scaffold=$(mktemp -d)
specify init "$scaffold/maestro-core" --integration copilot --script sh \
  --non-interactive --ignore-agent-tools
(cd "$scaffold/maestro-core" && find .specify .github -type f) | while read -r file; do
  [[ -e "$file" ]] || { mkdir -p "$(dirname "$file")"; cp "$scaffold/maestro-core/$file" "$file"; }
done
git status --short .specify .github
```

Expected: new template, script and prompt files; `constitution.md` unchanged
(`git diff --quiet .specify/memory/constitution.md && echo kept`).

- [x] **Step 2: Spec Kit sees the project**

Run: `specify check`
Expected: success, the Copilot integration listed.

- [x] **Step 3: Commit:** `docs: add the spec kit scaffolding`.

### T017 Root documents in English [US1, US4]

**Files:** rewrite `README.md`; create `AGENTS.md`;
modify `deny.toml`, `typos.toml`.

- [x] **Step 1: `README.md`**

````markdown
# maestro-core

[![CI](https://github.com/Orchestration-Maestro/maestro-core/actions/workflows/ci.yml/badge.svg)](https://github.com/Orchestration-Maestro/maestro-core/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/Orchestration-Maestro/maestro-core/badge)](https://scorecard.dev/viewer/?uri=github.com/Orchestration-Maestro/maestro-core)

The local runtime of Maestro: knowledge kernel, retrieval, orchestration and
the command-line tools. Today it holds one crate,
[`maestro-canonicalization`](crates/maestro-canonicalization/README.md), which
turns Markdown into provenance-bearing canonical documents, groups duplicates
and cuts them into token-budgeted chunks. The rest arrives slice by slice
([roadmap](docs/architecture/06-roadmap.md)).

## Start here

| To | Read |
| --- | --- |
| Understand the design | [docs/architecture](docs/architecture/README.md) |
| See why a choice was made | [docs/adr](docs/adr/README.md) |
| Learn the vocabulary | [CONTEXT.md](CONTEXT.md) |
| Follow the active slice | [specs](specs/000-foundation/spec.md) |
| Work in this repository | [AGENTS.md](AGENTS.md) |

## Develop

On Linux x86_64 with rustup:

```bash
scripts/bootstrap.sh   # the pinned toolbelt in .tools/, and the commit hooks
just check             # the local gate; it must pass before every push
just native            # the native tokenizer tests, through a local binding
```

The native tests read the tokenizer's artifacts where a binding file says they
are: see [TOKENIZER.md](crates/maestro-canonicalization/TOKENIZER.md).

## Licence

MIT: [LICENSE](LICENSE).
````

- [x] **Step 2: `AGENTS.md`**

```markdown
# Agent guide

The [constitution](.specify/memory/constitution.md) governs; this page is the
working summary.

| Command | When |
| --- | --- |
| `scripts/bootstrap.sh` | Once per clone: the pinned toolbelt and the commit hooks |
| `just check` | Before every push; it must exit 0 |
| `just native` | After touching the tokenizer; needs `MAESTRO_NATIVE_BINDING` |
| `just mutants` | Before a pull request whose diff CI cannot mutate within its 45 minutes |

1. Work from the active slice's `specs/NNN-*/tasks.md`; a changed behaviour
   starts with a failing test.
2. One pull request per repository per working session; a `feat` and a `fix`
   never share one.
3. Conventional titles, lower case after the type, subject at most 71
   characters, lines at most 80.
4. No personal path, secret or vendor-private material: `maestro-conventions`
   refuses them, and vendor material lives in the private collection (ADR-0009).
5. Files stay within 500 counted lines; functions within 100 lines,
   5 parameters and complexity 15. Split rather than allow.
6. Canonicalization identities and fixture bytes change only through a recorded
   decision.
7. Terms come from [CONTEXT.md](CONTEXT.md); a hard-to-reverse decision gets an
   ADR in [docs/adr](docs/adr/README.md).
```

- [x] **Step 3: The licence.** `LICENSE` alone covers the repository, which
  redistributes no third-party material.

- [x] **Step 4: `deny.toml` and `typos.toml`.** In `deny.toml`, replace the
  two-line comment above `unused-allowed-license` with
  `# A reviewed licence stays allowed while no current crate uses it.` In
  `typos.toml`, delete the entry for the Foundation standards' identifier prefix
  and its comment (there is no `docs/standards` here). Then run `typos`; add a word only when the repository
  means it, with a comment saying why.

- [x] **Step 5: Verify:** `cargo test -p maestro-conventions --test policies` passes
  (links from the new documents resolve); `typos` exits 0.

- [x] **Step 6: Commit:** `docs: write the repository's entry points in english`.

---

## Phase 5: CI, mutation and the local gate

### T018 GitHub configuration [US1]

**Files:** create `.github/workflows/{ci,scorecard,dependabot-auto-merge}.yml`,
`.github/dependabot.yml`, `.github/CODEOWNERS`.

- [x] **Step 1: `.github/workflows/ci.yml`**

```yaml
name: CI
"on":
  push:
    branches: [main]
  pull_request:
permissions:
  contents: read
jobs:
  # Keep the job id `rust`: the rust-ci-required ruleset requires the check
  # "rust / Required Rust CI" before any merge to the default branch.
  rust:
    uses: Orchestration-Maestro/rust-workflows/.github/workflows/ci.yml@3495c8391831b9f43b6b94f3b917ceaeb3cf64e3  # v1.2.1
    permissions:
      contents: read
    with:
      coverage-threshold: 90
      clippy-level: pedantic
      license-policy: enforce
      artifact-key: maestro-core
      # The import pull request's diff is the whole crate, 810 mutants, more
      # than the job's 45 minutes. A full local run reached zero survivors
      # first (specs/000-foundation/plan.md, Complexity Tracking); the next
      # pull request removes this line.
      mutation-test: false
  # Clippy and secret-scan findings into the Security tab: the only job that
  # needs a write scope.
  sarif:
    needs: rust
    uses: Orchestration-Maestro/rust-workflows/.github/workflows/upload-sarif.yml@3495c8391831b9f43b6b94f3b917ceaeb3cf64e3  # v1.2.1
    permissions:
      contents: read
      security-events: write  # upload SARIF to code scanning
    with:
      artifact-name: ${{ needs.rust.outputs.artifact-name }}
  # Coverage and test results into Codecov, logged in through OIDC: the only
  # job with id-token: write. The Codecov app is installed organization-wide.
  coverage:
    needs: rust
    uses: Orchestration-Maestro/rust-workflows/.github/workflows/upload-coverage.yml@3495c8391831b9f43b6b94f3b917ceaeb3cf64e3  # v1.2.1
    permissions:
      contents: read
      id-token: write  # log in to Codecov without a stored token
    with:
      artifact-name: ${{ needs.rust.outputs.artifact-name }}
```

- [x] **Step 2: Copy the others**

```bash
ORG=~/workspace/Orchestration-Maestro/.github
RW=~/workspace/Orchestration-Maestro/rust-workflows
cp "$ORG/workflow-templates/scorecard.yml" .github/workflows/scorecard.yml
sed -i 's/\$default-branch/main/' .github/workflows/scorecard.yml
cp "$RW/.github/workflows/dependabot-auto-merge.yml" .github/workflows/
cp "$RW/.github/dependabot.yml" .github/dependabot.yml
printf '%s\n' '* @FrancoisLDaigneault' > .github/CODEOWNERS
```

  In `.github/dependabot.yml`, replace the cargo entry's `directories:` list with
  `directory: /`.

- [x] **Step 3: Lint**

```bash
actionlint && zizmor --offline --persona=pedantic --no-progress --config .github/zizmor.yml .github/
yamlfmt -no_global_conf -lint
```

Expected: no findings.

- [x] **Step 4: Commit:** `ci: call rust-workflows v1.2.1 and the organization's checks`.

### T019 Full local mutation run [US1]

**Files:** create `.cargo/mutants.toml` only if an equivalent mutant needs an
exclusion; tests where survivors show gaps.

- [x] **Step 1: Run every mutant**

Run: `just mutants 8 2>&1 | tail -5`
Expected at the end: `N mutants tested`, with `0 missed` and `0 timeouts`.

- [x] **Step 2: Kill each survivor.** For each line in `mutants.out/missed.txt`,
  write a test named after the behaviour the mutant breaks, in the test module
  of the mutated file; see it fail under the mutant, then rerun that function's
  mutants: `cargo mutants --file <file> --re '<function>' --cargo-arg=--locked`.
  A mutant that changes no observable behaviour goes in `.cargo/mutants.toml`
  as `exclude_re = ["<exact mutant name>"]` with a comment giving the reason;
  each exclusion is reviewed in the pull request.

- [x] **Step 3: Record the evidence.** Save the final summary line and the date
  in the pull request body (T022), and keep `mutants.out/outcomes.json` with the
  archive: `cp mutants.out/outcomes.json "$ARCHIVE/mutants-outcomes.json"`.

- [x] **Step 4: Commit:** `test: kill every surviving mutant`.

### T020 The local gate passes [US1]

- [x] **Step 1: Run it**

Run: `just check; echo "exit=$?"`
Expected: `exit=0`, the coverage summary at 90 % or more.

- [x] **Step 2: Re-verify the baseline**

```bash
(cd "$CRATE" && sha256sum --check --quiet "$ARCHIVE/fixtures.sha256") && echo "fixtures ok"
just native 2>&1 | grep 'test result'
```

Expected: `fixtures ok`, native tests ok.

---

## Phase 6: User Story 3 — the repositories S1 needs

### T021 Publish `maestro-core` [US3]

- [x] **Step 1: Confirm with the maintainer:** "Create the public repository
  Orchestration-Maestro/maestro-core and open the import pull request?"

- [x] **Step 2: Create it and apply the checklist**

```bash
gh repo create Orchestration-Maestro/maestro-core --public --add-readme \
  --description "The local runtime of Maestro: knowledge kernel, retrieval and orchestration"
gh api -X PATCH repos/Orchestration-Maestro/maestro-core/properties/values \
  --input - <<< '{"properties":[{"property_name":"stack","value":"rust"}]}'
gh api -X PATCH repos/Orchestration-Maestro/maestro-core -F allow_auto_merge=true
```

  The two web-only items of the `.github` repository's new-repository checklist
  (reported content, social preview) go to the maintainer.

- [x] **Step 3: One clean commit on top of GitHub's first**

```bash
git remote add origin git@github.com:Orchestration-Maestro/maestro-core.git
git fetch origin main
git switch --create feat/foundation origin/main
git restore --source=work/foundation --staged --worktree -- .
git status --short | head
just check
```

  Commit with the title `feat: start maestro-core from the canonicalization crate`
  and a body naming what S0 changed and the mutation evidence of T019.

- [x] **Step 4: Prove the pushed history is clean**

```bash
cargo test -p maestro-conventions --test policies 2>&1 | grep 'test result'
gitleaks git --no-banner --redact --log-opts="origin/main..HEAD"
git log --oneline origin/main..HEAD | wc -l
```

Expected: `ok. 5 passed`, no leaks, `1`.

- [x] **Step 5: Push and open the pull request**

```bash
git push -u origin feat/foundation
gh pr create --title "feat: start maestro-core from the canonicalization crate" \
  --body-file <(git log -1 --format=%b)
```

- [x] **Step 6: Merge when green.** `rust / Required Rust CI`, CodeQL, SARIF and
  coverage pass; squash-merge with the branch deleted. Then check `main`: the CI
  run on the push and the Scorecard run pass.

### T022 Check `maestro-model-router` [US3]

The router was imported on 2026-09-24 (pull request #1, merged): this task only
checks it.

- [x] **Step 1: Verify its settings and its last run**

```bash
R=Orchestration-Maestro/maestro-model-router
gh api "repos/$R" --jq '"\(.visibility) \(.default_branch)"'
gh api "repos/$R/properties/values" --jq '.[] | "\(.property_name)=\(.value)"'
grep -rhoE 'rust-workflows/[^@ ]+@[0-9a-f]{40}' ~/workspace/Orchestration-Maestro/maestro-model-router/.github/workflows | sort -u
gh run list --repo "$R" --branch main --limit 5 --json name,conclusion \
  --jq '.[] | "\(.name): \(.conclusion)"'
```

Expected: `public main`, `stack=rust`, every pin at
`3495c8391831b9f43b6b94f3b917ceaeb3cf64e3`, and no failed run on `main`.

### T023 Create `ctm-collection` [US3]

- [x] **Step 1: The maintainer confirms the location** (08 §17; default: a
  private repository on the maintainer's account, since the organization
  refuses private repositories).

- [x] **Step 2: Create it and add the staged sources**

```bash
gh repo create FrancoisLDaigneault/ctm-collection --private --add-readme \
  --description "Private Control-M collection: sources, policy and evaluation data"
cd ~/workspace/Orchestration-Maestro
gh repo clone FrancoisLDaigneault/ctm-collection
cd ctm-collection
git switch --create docs/sources
cp -r "$ARCHIVE/ctm-collection/." .
```

  Write its `README.md`: what it holds (vendor-specific sources, the corpus
  sampler, later the exporter, `collection.json`, the quality ledger and the
  private evaluation set), that it is private by ADR-0009, and that no corpus
  content is committed.

- [x] **Step 3: Pull request and merge**, titled `docs: add the private collection sources`.

### T024 Mutation-test pull requests again [US1] (next session)

- [x] **Step 1:** In `maestro-core`'s `.github/workflows/ci.yml`, delete the
  `mutation-test: false` line and the comment above it.
- [x] **Step 2:** `just check`; pull request titled
  `ci: mutation-test pull requests again`; its diff has no Rust change, so the
  mutation step finishes in seconds.
- [x] **Step 3:** Merge when green; remove the row from plan.md's Complexity
  Tracking in the same pull request.

### T025 Split oversized units without tiny last pieces [US1] (before S1)

The maintainer decided on 2026-09-24 to fix this in its own pull request
before S1 indexes any content, since the cuts decide chunk identities.

Today a paragraph of 300 words (1,499 bytes) splits at 365, 925 and 1,490
bytes, then into `word ` and `word`; a paragraph of sentences ends with
`Sentence `, `one ` and `here.`. `fit_prefix` cuts back to the last preferred
boundary even when the whole rest fits, and `split_unit` then backs off one
more whitespace. T019 pins these cuts as they are.

- [x] **Step 1:** Write the expected cuts first in
  `chunk_split/tests/splitting.rs`: once the rest of a unit fits, it is one
  last piece. See the tests fail.
- [x] **Step 2:** Fix `fit_prefix` and `split_unit`; `just check` and the
  native tests pass, and chunk identities change once.
- [x] **Step 3:** A full mutation run keeps 0 missed and 0 timeouts; pull
  request titled `fix: split oversized units without tiny last pieces`.

---

## Dependencies

T001 → T002 → T003 → T004 → T005 → T006–T010 (any order) → T011 → T012 →
T013 → T014 → T015 → T016, T017 (either order) → T018 → T019 → T020 → T021.
T022 needs nothing; T023 needs only T001; T024 follows T021 in a later session.
T025 follows T021 and comes before S1.

## Spec coverage

| Requirement | Tasks |
| --- | --- |
| FR-S0-001 archive | T001 |
| FR-S0-002 product code and policy crate | T002, T005 |
| FR-S0-003 lints inherited | T004 |
| FR-S0-004 API, identities, fixtures | T006–T014 (verified each time), T015 |
| FR-S0-005 source limits | T005–T012 |
| FR-S0-006 binding | T015 |
| FR-S0-007 CI | T018, T024 |
| FR-S0-008 repository files | T003, T017, T018 |
| FR-S0-009 repositories | T021, T022, T023 |
| FR-S0-010 no private material | T002, T005, T015, T021 Step 4, T022 Step 1 |
| FR-S0-011 policies as tests | T005 |
| FR-S0-012 mutation evidence | T019, T024 |
| US4 Spec Kit and links | T016, T017 |
