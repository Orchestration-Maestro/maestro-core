# Reverse engineering produces knowledge only, behind a clean-room boundary

Status: accepted, 2026-09-24.

Maestro reimplements the outcomes of the researched providers rather than
embedding their engines ([04 §1](../architecture/04-intelligence-backend.md#1-scope-and-stance)),
and replaces each one only when a native capability passes an evaluation
([04 §11](../architecture/04-intelligence-backend.md#11-transition-from-existing-providers)).
Learning what a system does is therefore a standing need, and the way it is done
decides whether the result is ours to ship.

Analysis output is knowledge in the kernel and never reaches an implementation.
Two scopes carry it: `analysis/<target>` holds captures, findings, traces and
decompiled text; `implementation/<feature>` holds the repository and the agents
that write code. The only bridge is a promoted **specification** — observable
inputs, outputs, formats, errors and acceptance scenarios, quoting nothing —
approved by a named person and checked for verbatim spans. Every component we
adopt, port or study is registered first with its origin, pinned revision,
licence, usage class and obligations, and an unresolved licence refuses the
analysis. The instruments (ScanCode, Joern, CodeQL, tree-sitter, Ghidra, Frida,
mitmproxy, Playwright) are sandboxed `analyzer` extensions behind one contract,
never runtime dependencies and never shipped. Design:
[09](../architecture/09-reverse-engineering.md).

## Considered options

- Read the sources and port what is useful: fastest, but a port is a derivative,
  so the original licence and its obligations travel into public MIT
  repositories. Kept only as the explicit `ported` usage class, with notices.
- Decompile first, with Ghidra as the primary instrument: loses the names,
  types, tests, structure and rationale that source analysis keeps, answers none
  of the design questions, and produces the material most dangerous to have near
  an implementation. Narrowed to lawfully possessed native components.
- Trust process discipline instead of scopes: unverifiable after the fact. An
  assertion that a clean room was kept is worth nothing without a record of who
  held which scope and what crossed the gate.
- A separate analysis tool and store outside the kernel: a second authority,
  second pipeline and second access model, against
  [ADR-0002](0002-one-authority-many-projections.md).

## Consequences

Analysis is slower than copying, and deliberately so: a component cannot be used
before its licence is resolved, and a specification cannot cross the boundary
without an approver. In exchange, every capability we ship can name what it was
derived from and what that obliges, and the behaviour contracts produced along
the way become the parity gates that decide when a provider is actually
replaced.
