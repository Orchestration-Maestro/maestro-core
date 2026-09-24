# The workbench is a native Rust desktop application

Status: accepted (owner-confirmed direction, recorded 2026-09-24).

The people-facing workbench of the intelligence backend is a native Rust desktop
application over the same persistent runtime that serves the CLI and MCP, not a
web application in a desktop wrapper, a webview or a WebAssembly front end.
Closing a window never cancels runtime-owned work; the runtime owns the data and
the GUI is a client over versioned, authenticated local IPC with reconnection.
egui/eframe is the proposed toolkit, confirmed when I4 starts; Linux, macOS and
Windows are first-class targets for this product, each qualified before it is
claimed.

## Considered options

- Local web front end: faster to prototype, but contradicts the confirmed
  direction and adds a browser runtime to the trust boundary.
- Embedding the researched visual tools (Archify renderer): their useful
  outcomes are reimplemented natively; their JavaScript runtime is not reused.
