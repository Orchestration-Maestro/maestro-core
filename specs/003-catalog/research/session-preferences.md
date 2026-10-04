# Session preferences delivery: C05e handoff

Requirements: FR-S3-031, FR-S3-036, SC-S3-010.

## S3 contract and evidence

`Session::mcp_context()` builds one English fragment from the already validated
snapshot and adds path-free provenance. The existing CLI-to-MCP immutable string
is retained; no second file read, discovery rule or preference store is added.
`ClientPreferencesDelivery` is the replaceable port; `McpPreferencesDelivery`
puts that string in the standard initialization instructions. There are no
client-name branches, launchers, plugins or dynamic discovery. A fifth client
adds an adapter, not caller branches.

Only canonical quoted language tags (or `"auto"`) and the registered quoted tone
enter the fragment. `"auto"` follows the question language. The fixed rule is:

> Keep code, commits, names, identifiers, logs and documentation in English.

`native_preferences_instructions()` prepares value-free native adapter input:
follow the current MCP session's conversation language and tone, plus exactly
that fixed rule. Native files must never persist resolved language/tone values.
C06/C07 consume this input for projections; C05e does not change their files.

`settings::tests::instructions` checks canonical construction for each tone,
missing/mistyped values, noncanonical tags, instruction injection and the native
input. `catalog_client_preferences` inspects complete actual stdio initialization
results for Pi, Claude Code, Codex and Copilot fixtures in nested and empty
folders, with/without `--workspace`, user/default/external-workspace fallback,
explicit language flags, restart after edits and model replacement refusal.
An unsupported interface language retains its tag while the visible CLI note
says the interface is English. No localization prose enters the MCP payload.

MCP roots capability, list-changed notifications and an unsolicited roots result
cannot change the process-selected workspace. The server does not request roots:
roots arrive after initialization and there is no standard instructions-changed
notification. S3 reads only explicit `--workspace` or user preferences/defaults.
These synthetic fixtures prove bytes and delivery, not real host consumption or
obedience. C08 supplies the separately qualified host evidence.

## Named S4 session-preferences launch-adapter obligation

For both provider routes, inspect the actual instruction payload for each
launch, resume and delegation, across all three tones and a non-interface
language such as `ja`. Require the shared fragment, canonical tag, immutable
session provenance and English artifact/log rule. Missing/stale fragments and
model-supplied preference replacements must fail payload tests. Session restart
may observe edits; a running session must not silently switch snapshots.

Effect tests must observe generated conversation in the selected language/tone
while code, commits, names, identifiers, logs and documentation remain English.
A UI fallback must be visible without changing conversation language. Fixture
bytes alone cannot close these launch/consumption/effect obligations.

## Named S4 workspace-trust hook obligation

Transferred C20 owns Copilot qualification (4 h). Pi, Codex and Claude Code each
also need qualified trusted-event/identity adapters through the shared
`WorkspaceTrust` port. For every host, inspect normalized event payloads and
allow/deny/error-to-deny receipts, then observe filesystem/executor effects:

- Deny writes outside trusted roots; allow the corresponding contained write.
- Deny secret reads even inside trust; allow a non-secret read neighbour.
- Deny link escapes; allow a contained path neighbour.
- Deny agent-shell trust administration, even with correct `--confirm-path`.
- Deny adapter errors with zero executor calls; record unsupported hooks as
  unprotected, never as a successful enforcement check.

Every stimulus must reach the real guard. Hold the shared update lifecycle lease
at task/session admission. No S3 preferences or native instruction file confers
trust or proves containment. These obligations are also recorded in
[06](../../../docs/architecture/06-roadmap.md) and
[08](../../../docs/architecture/08-traceability.md).

## Limits and dependencies

No library, size bound, timeout, budget or configuration default is added.
The existing S1 canonical-language parser and already validated session layers
are reused unchanged; no new Pi-equivalent limit is needed.
