# Agent guide

The organization's [golden rules](https://github.com/Orchestration-Maestro/.github/blob/main/golden-rules/engineering.md) come first,
and [`docs/standards/`](docs/standards/engineering.md) maps them to this
repository; this page is the working summary.

| Command | When |
| --- | --- |
| `rust-gate setup` | Once per clone: the pinned toolbelt and the commit hooks ([README](README.md#-develop)) |
| `just check` | Full local CI check for the normal workflow; active slice branches follow the [S1 integration workflow](specs/001-knowledge-kernel/tasks.md#current-s1-integration-workflow) |
| `just native` | After touching the tokenizer; needs `MAESTRO_NATIVE_BINDING` |
| `just mutants` | Full-workspace mutations; defaults to four jobs. Use `just mutants 1` on memory-constrained machines. Follow the active slice for diff-scoped mutation runs. |

1. Work from the active slice's `specs/NNN-*/tasks.md`; a changed behaviour
   starts with a failing test.
2. Outside an explicitly documented slice workflow, one pull request per
   repository per working session; a `feat` and a `fix` never share one. An
   active slice branch uses one integration pull request; see the linked S1
   workflow above.
3. Conventional titles, lower case after the type, subject at most 71
   characters, lines at most 80.
4. No personal path, secret or vendor-private material. `maestro-conventions`
   refuses personal paths and private registry or quality-service settings,
   and gitleaks refuses secrets. No check refuses vendor text yet (it comes
   after M1), so keep it out by hand: vendor material lives in the private
   collection (ADR-0009), and public tests use synthetic fixtures only.
5. Files stay within 500 counted lines; functions within 100 lines,
   5 parameters and complexity 15. Split rather than allow.
6. Canonicalization identities and fixture bytes change only through a recorded
   decision.
7. Terms come from [CONTEXT.md](CONTEXT.md); a hard-to-reverse decision gets an
   ADR in [docs/adr](docs/adr/README.md).
8. Test scratch directories come from
   `maestro_test_scratch::scratch_directory()`, which prefers the tmpfs
   `/dev/shm`: the kernel's durable writes wait on `fsync`, and on a disk the
   suite took 147 s against 40 s on tmpfs. `MAESTRO_TEST_SCRATCH`, an
   absolute path, names another base. Programs a test runs go under
   `disk_scratch_directory()`, since tmpfs is often mounted `noexec`.
