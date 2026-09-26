# Agent guide

The organization's [golden rules](https://github.com/Orchestration-Maestro/.github/blob/main/golden-rules/engineering.md) come first,
and [`docs/standards/`](docs/standards/engineering.md) maps them to this
repository; this page is the working summary.

| Command | When |
| --- | --- |
| `rust-gate setup` | Once per clone: the pinned toolbelt and the commit hooks ([README](README.md#develop)) |
| `just check` | Before every push, which runs it too: exactly what CI runs; it must exit 0 |
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
