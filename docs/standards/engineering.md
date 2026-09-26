# Engineering rules in `maestro-core`

`maestro-core` follows the organization's [engineering
rules](https://github.com/Orchestration-Maestro/.github/blob/864d85597a833864cd8506c3925830503b3c2163/golden-rules/engineering.md).
This page is its rule map (C-001): for every rule, what holds it here, or why it
does not apply. A row may name a stricter local rule; none weakens one.

`rust-gate rules` writes the rows from the golden rules of `.github@864d855` at
every commit and keeps what each row says here. A rule added there arrives as
"Not mapped yet", and the daily drift check reports it until it is mapped.

## Rule map

| Rule | Held here by |
| --- | --- |
| FND-001 Think before coding | Review: the pull request states its assumptions, the alternatives weighed and what stays unclear |
| FND-002 Simplicity first | Review: the pull request names the requirement each change serves; nothing speculative lands |
| FND-003 Surgical changes | Review: every changed line traces to the pull request's goal |
| FND-004 Goal-driven execution | Review: the pull request's Verification section holds the check that means done, and its output |
| P-001 YAGNI | Review: a reviewer names the principle a change breaks |
| P-002 KISS | Review: a reviewer names the principle a change breaks |
| P-003 DRY | Review: a reviewer names the principle a change breaks |
| P-004 WET | Review: a reviewer names the principle a change breaks |
| P-005 Rule of three | Review: a reviewer names the principle a change breaks |
| P-006 Chesterton's fence | Review: a reviewer names the principle a change breaks |
| P-007 Boy Scout rule | Review: a reviewer names the principle a change breaks |
| P-008 Least astonishment | Review: a reviewer names the principle a change breaks |
| P-009 Single responsibility | Review: a reviewer names the principle a change breaks |
| P-010 Composition over inheritance | Review: a reviewer names the principle a change breaks |
| P-011 Fail fast | Test: unsafe nesting, invalid UTF-8 and malformed sidecars are refused at the boundary (`cli_contract`, `validation_boundary`); the CLI exits 1 |
| P-012 Make illegal states unrepresentable | Review: a reviewer names the principle a change breaks |
| P-013 Parse, don't validate | Code: input becomes typed values (`CanonicalDocument`, sidecars with unknown fields refused); a saved snapshot is rebuilt and compared, never trusted |
| P-014 Principle of least privilege | CI: each job holds only its scope (`sarif` alone writes security events, `coverage` alone gets an OIDC token); zizmor checks every workflow |
| P-015 Separation of concerns | Review: a reviewer names the principle a change breaks |
| P-016 Zero, one or many | Review: a reviewer names the principle a change breaks |
| P-017 Premature optimisation | Review: a reviewer names the principle a change breaks |
| P-018 Broken windows | Gate: Clippy pedantic and rustdoc warnings are errors, zero surviving mutants, every file within 500 counted lines |
| ENF-001 No machine-named paths | Test: `no_personal_path_in_repository_text` fails on a home or profile path; the tokenizer's artifacts resolve through `MAESTRO_NATIVE_BINDING` |
| ENF-002 Every claimed platform is tested | CI: every pull request runs the gate on `ubuntu-24.04` and `cargo test` on `macos-15` and `windows-2025`; the README claims those three ([ADR-0018](../adr/0018-rustix-on-unix-and-win32-flags-on-windows.md)) |
| ENF-003 English only | Review: prose and identifiers are English |
| ENF-004 Conventional commits | Organization: the `commits-are-conventional` ruleset refuses any other title on the default branch |
| ENF-005 Failing test first | Review: the new test is seen failing first; CI mutation-tests every pull request's diff, so an untested change fails |
| ENF-006 Never weaken a gate | Organization: required checks and code scanning block every merge; a gate changes only in its own reviewed pull request |
| ENF-007 Pull requests only | Organization: the `default-branch-discipline` ruleset (pull request, signed commits, code scanning) and `floor-no-destruction`, with no bypass actor |
| ENF-008 Tiered checks | Commit hooks (prek), then `just check` before a push, then CI's required gate; `just check`, which the pre-push hook runs, is CI's checks job itself (`rust-gate ci --local`), and the full mutation run and native tests run by hand |
| ENF-009 Allowlists that cannot rot | Test: each `.cargo/mutants.toml` exclusion names one mutant by line and its reason, and returns as missed when the code moves; typos and zizmor entries carry their reasons |
| ENF-010 Configuration is the authority | Organization: its settings as code in `.github/org/`, checked weekly by `org-drift.yml` |
| ENF-011 Instructions grant nothing | Organization: authority lives in rulesets, workflow `permissions:` and access control; no instruction file grants any |
| ENF-012 Pinned inputs | Gate: `Cargo.lock` with `--locked`, actions pinned by SHA (zizmor hash-pin), tools pinned by rust-workflows and installed by `rust-gate setup`, tokenizer artifacts by size and SHA-256 |
| ENF-013 No secret in history | Organization: secret scanning with push protection and validity checks (`maestrolabs-baseline`); CI: gitleaks |
| ENF-014 Multi-factor authentication | Organization: two-factor authentication is required of every member and outside collaborator |
| C-001 Map every rule | These pages, kept current by `rust-gate rules` at every commit; the daily drift check reports a row not mapped yet |
| C-004 Detect drift | Organization: the daily drift check opens a `Drift:` issue for this repository |
| C-005 Keep the evidence | GitHub: pull requests, CI runs with their reports, and drift issues |
| C-006 Controlled exceptions | Review: an exception is recorded in the pull request that makes it, with its scope, rationale and expiry |
