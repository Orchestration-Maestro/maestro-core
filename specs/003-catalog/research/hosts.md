# Catalog host format probe (C01)

Measured 2026-09-28 under owner approval OA2, on the hosts already installed
on the owner's Linux (WSL) machine. No host was installed or upgraded and no
account was used. The probe's own runs, version checks included, read and
wrote no real host home or configuration.

Outside the probe, the first lane ran `copilot --version`, `pi --version`,
`claude --version` and `codex --version` once against the real homes before
the sandbox existed, and one more `codex --version` as a deliberate control.
Codex 0.150.1 then creates a helper directory of its own under
`~/.codex/tmp/arg0/` (an empty `.lock` and symbolic links to its binary) and
prunes stale ones on its next run; `codex-arg0hOJBSN` from that control is
left for Codex to prune. No configuration changed.

## Result

- **Agents: use the ADR-0005 sidecar.** Copilot CLI 1.0.88 loads an agent
  profile that carries a `metadata:` block, but it drops the block and logs
  `unknown field ignored: metadata` at every session start. For Copilot CLI
  `metadata:` is an unsupported field, and FR-S3-002 forbids masking one. A
  plain `<name>.agent.md` beside `<name>.maestro.toml` loads with no
  diagnostic at all, in the same directory.
- **Skills: the probe cannot tell.** A `SKILL.md` with a `metadata:` block
  loads with no diagnostic on Copilot and Pi, but neither host reports an
  unknown skill key at all: a control skill with an unknown top-level
  `owner:` key also loads with no diagnostic on Copilot, and Pi's skill
  loader reads no extra key (source read). Silence is therefore no evidence
  of support. `metadata` is a field of the
  [Agent Skills specification](https://agentskills.io/specification) (an
  optional key-value map), which Pi documents implementing
  (`docs/skills.md` of Pi 0.87.1); a skill `metadata:` choice rests on that
  specification, not on this probe.
- **Pi needs its own projection.** pi-subagents reads agent frontmatter with
  a line-based parser, not YAML, and uses its own tool syntax; the Copilot
  profile is not a valid Pi agent. The projection names tools, the MCP
  provider extension, the model and skills explicitly (mapping below). The
  child runs on the model its projection names, not its parent's, and an
  unknown model stops the child instead of falling back.
- **Shadows are silent and opposite.** A same-name user agent beats the
  project agent on Copilot; the project agent beats the user agent on Pi.
  Neither host reports it (asserted on Copilot's log and Pi's standard
  error), so Maestro must detect it (the probe does).
- **Claude Code and Codex**: MCP registration guidance only, as FR-S3-007
  says. Claude Code connects a local-scope registration; Codex records one
  but never connects in `codex mcp`, so its live call was not run.

The supervisor rules on the C03 format from this evidence; this note does
not switch any shape by itself.

## Pins

The digest covers the code that runs, not a launcher script: a native
binary for Claude Code and Codex, the platform binary that Copilot's npm
loader launches, and the whole npm package for Pi, whose `cli.js` only
loads the bundle beside it.

| Host | Version (exact first line) | Code the digest covers | SHA-256 |
| --- | --- | --- | --- |
| GitHub Copilot CLI | `GitHub Copilot CLI 1.0.88.` | `@github/copilot/node_modules/@github/copilot-linux-x64/copilot`, launched by `npm-loader.js` | `0059754cf78c3f3bf2c9d4564dfa7e9e25f3a3f8f411f2f0cdad9363f5662748` |
| Pi | `0.87.1` | package tree of `@earendil-works/pi-coding-agent` (entry `dist/bundle/cli.js`) | `af27c543eec68c35e76fa1b418c01f27eac5c33cef1b8b96e15950de330442b3` |
| Claude Code | `2.1.283 (Claude Code)` | `claude/versions/2.1.283` | `1859583ce32920595c61ef868bee52e1b1594f7486db209935e01f1e5e804ae2` |
| Codex | `codex-cli 0.150.1` | `0.150.1-x86_64-unknown-linux-musl/bin/codex` | `abf1bb1643a79f73aa78ee627e111e02d4f8c98f25813a0cf6ce277709664386` |

A package tree digest is what this recipe prints, run in the package's
directory (the nearest one above the script holding `package.json`):

```sh
find . -type f -not -path './node_modules/*' | LC_ALL=C sort \
  | xargs sha256sum | sha256sum
```

Regular files only (symbolic links are neither followed nor counted), the
top-level `node_modules` left out. `code_digests::tree_digest` computes the
same value, and a test checks it against the recipe's output on a synthetic
tree.

Pi's installed providers, loaded read-only by path with `pi -e`, never
installed:

| Package | Version | `index.ts` SHA-256 | Package tree SHA-256 (recipe above) |
| --- | --- | --- | --- |
| pi-mcp-adapter | 2.37.0 | `f0624b73490ed446d0f1045f606347247abbf2eb6575afaaade7cfdad12b3264` | `6f972eaeb97ce09d6fa007ac4e20ac91ae9bae59972159303fd552989d06558f` |
| pi-subagents | 0.64.0 | `a2f11dbe8e200bd8c590441316a9c6ab222318d2aa738670dedcf0e72592dde3` | `9a062dab0ecd8234cf0e5962bfe3797583655b2d9a513cea526cbd8bfc8f5949` |

Node.js v24.20.0 runs Copilot's loader and Pi. The probe checks each host's
version line in a sandbox before it runs and prints the digest of the code
it runs; a missing host, another version or a missing provider blocks the
probe, and a blocked probe fails its test. A changed pin needs this probe
again.

## Method

`crates/maestro/tests/it/catalog_host_probe/` runs each host as an ignored
test:

```sh
~/.local/bin/capped cargo test -p maestro --test it catalog_host_probe -- \
  --ignored --nocapture
```

- Each run gets a new temporary directory: `HOME`, the host's own home
  (`COPILOT_HOME`, `PI_CODING_AGENT_DIR`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`)
  and the project. The environment is cleared except `PATH`, so no variable
  of the calling session reaches the host.
- The model is a scripted OpenAI-compatible endpoint on the loopback
  interface (Copilot's offline BYOK mode, Pi's `models.json`). It makes one
  planned tool call and records every request, so the probe reads the exact
  prompt and tool list the host sent.
- The MCP server is the built `maestro mcp` over the public synthetic
  glossary, published in a scratch Maestro home; a passing call returns a
  `maestro-evidence/1` result.
- Fixtures: `tests/fixtures/catalog/hosts/{metadata.agent.md,
  sidecar.agent.md,sidecar.maestro.toml,pi.md}`, synthetic only.

Pi's pi-mcp-adapter also reads `~/.config/mcp/mcp.json` and
`~/.agents/mcp.json`, and pi-subagents disables itself when
`PI_SUBAGENT_CHILD=1` is inherited: a probe that kept the real `HOME` or the
caller's environment would measure the owner's setup, not the catalog.

## Results

Status words: **passed** (observed and asserted by the probe),
**observed, not asserted** (seen in an exploration run, the help text or
the host's source, not checked by a test), **unsupported** (the host does
not support it), **not run** (not measured here).

### GitHub Copilot CLI 1.0.88

| Check | Status | Observation |
| --- | --- | --- |
| `metadata:` agent profile | passed, field unsupported | Loads; log: `.github/agents/probe.agent.md: unknown field ignored: metadata`; no metadata text in the prompt |
| Sidecar agent profile | passed | `probe.agent.md` plus `probe.maestro.toml` in `.github/agents/`: no diagnostic names either file, and no `unknown field` line at all |
| Other unknown key (`owner:`) | observed, not asserted; field unsupported | Same warning shape, `unknown field ignored: owner` (exploration) |
| Malformed frontmatter | observed, not asserted | Exit 1: `custom agent markdown frontmatter is malformed` (exploration) |
| Exact tools | passed | `tools: ["maestro/knowledge_search", "view"]` offers exactly `maestro-knowledge_search` and `view`, plus `skill` and `sql`, which Copilot always adds |
| Real MCP call | passed | `maestro-knowledge_search` returned `maestro-evidence/1` |
| Workspace MCP, trusted | passed | With `COPILOT_ALLOW_ALL=true`, which trusts the folder in prompt mode, `.mcp.json` loads and its tool is offered |
| Workspace MCP, untrusted | observed, not asserted | Without the trust no MCP tool was offered (exploration); the help says truthy values other than `true` only approve tools |
| Skill with `metadata:` | passed | `.agents/skills/probe-skill/SKILL.md` reaches the prompt; no log line names it |
| Control skill with unknown `owner:` | passed; unknown skill keys not reported | `.agents/skills/control-skill/SKILL.md` reaches the prompt; no log line names it. Copilot does not report unknown skill keys, so the previous row is no evidence of `metadata:` support |
| Identity | passed | The `name:` field, not the file name: a user `renamed.agent.md` with `name: probe` shadows the project `probe` |
| Same-name shadow | passed | The user agent (`$COPILOT_HOME/agents`) wins over `.github/agents`; no log line names either file |
| Lookup | observed, not asserted | Agents and skills are read at session start (each probe is a new session) |
| Reload inside a running session | not run | Only new sessions were measured |

### Pi 0.87.1 with pi-subagents 0.64.0 and pi-mcp-adapter 2.37.0

| Check | Status | Observation |
| --- | --- | --- |
| Projected agent (`pi.md`) | passed | The child's prompt holds the agent body |
| Explicit child model | passed | The parent runs on `probe-model`; every child request names `probe-child-model`, the model `pi.md` names |
| Unknown child model | passed; no fallback | With `model: probe/absent-model` the child never sends a request, and the parent's tool result says `Unknown subagent model 'probe/absent-model' in the active Pi model registry.` |
| Exact tools | passed | `tools: read, mcp:maestro/knowledge_search` offers exactly `read` and `maestro_knowledge_search`, plus `contact_supervisor`, which pi-subagents adds |
| Real MCP call | passed | The child's `maestro_knowledge_search` returned `maestro-evidence/1` |
| Provider absent | passed | Without `extensions:` naming pi-mcp-adapter the child refuses: `requested unavailable child tools: maestro_knowledge_search` |
| Skill | passed | `skills: probe-skill` puts the shared `.agents/skills` skill in the child's prompt |
| Same-name shadow | passed | The project agent (`.pi/agents`) wins over the user agent; Pi's standard error names neither file |
| Cold MCP metadata cache | passed | Every probe starts with an empty agent directory; direct MCP tools still reach the child |
| Unknown agent frontmatter keys | observed, not asserted; silent | pi-subagents keeps unknown keys as `extraFields` without a diagnostic (source read, `src/agents/agents.ts`) |
| Skill `metadata:` and other extra skill keys | observed, not asserted; ignored | Pi's skill loader (`loadSkillFromFile` in `dist/bundle/chunks/`) reports only read or parse failures and name or description problems; it neither reads nor reports `metadata` or any other extra key (source read) |
| Reload inside a running session | not run | Only new sessions were measured |

### Claude Code 2.1.283 and codex-cli 0.150.1

| Check | Status | Observation |
| --- | --- | --- |
| Claude Code, project `.mcp.json` | passed | `claude mcp list` shows `Pending approval`: a project file needs the user's interactive approval |
| Claude Code, `mcp add-json --scope local` | passed | `claude mcp list` shows `✔ Connected` after a real MCP handshake |
| Codex, `codex mcp add` | passed | `codex mcp get --json` shows the stdio command, arguments and environment |
| Codex connection or call | not run | `codex mcp` never starts the server; a live call needs a model session, which FR-S3-007 does not require |
| Agent projection | not applicable | Registration guidance only |

## Pi mapping

| Copilot profile | Pi projection | Note |
| --- | --- | --- |
| `name`, `description` | same | |
| body | body | Same text |
| `tools: ["<server>/<tool>"]` | `tools: mcp:<server>/<tool>` | The child sees `<server>_<tool>` |
| `tools: ["view"]` | `tools: read` | Built-in names differ per host |
| MCP server in `.mcp.json` | same file, plus `"directTools": true` | Copilot ignores the extra key |
| (none) | `extensions: <pi-mcp-adapter>/index.ts` | Required: tool names alone do not load the provider |
| (none) | `model: <provider>/<model>` | Explicit: the child runs on it, not on its parent's model, and an unknown model stops the child |
| skill in `.agents/skills` | `skills: <name>` | Both hosts discover `.agents/skills` |
| sidecar metadata | not projected | Maestro reads it; Pi reports no unknown key |

## Receipts

Red, first round, with the hosts off `PATH` (all eight live cases then):
`blocked: copilot is not on PATH`, `blocked: pi is not on PATH`,
`blocked: claude is not on PATH`, `blocked: codex is not on PATH`.
Red, with `MAESTRO_PROBE_PI_PACKAGES=/nonexistent` (three Pi cases):
`blocked: pi-mcp-adapter is not installed in /nonexistent`.

Red, review round (before the fixture named its own child model): the
explicit-model case failed with the child on `["probe-model", "probe-model"]`,
and the unknown-model case with `the child ran on ["probe-model",
"probe-model"]`. The control-skill case was first written expecting Copilot
to report the unknown key; it failed with `the control skill's unknown key
drew no diagnostic`, and the test now pins the observed silence.

Green (2026-09-28, review round): `test result: ok. 9 passed; 0 failed` in
10 s. Probe output lines:

```text
copilot metadata shape: loaded; `metadata` reported as an unknown field
copilot sidecar shape: loaded without a diagnostic
copilot shadow: the user agent wins silently; the probe detects it
pi projection: exact tools, explicit provider, model and skill; MCP call passed
pi absent child model: the child does not run; no fallback model:
  Unknown subagent model 'probe/absent-model' in the active Pi model registry.
pi without its MCP provider: the child fails closed
pi shadow: the project agent wins silently; the probe detects it
claude code: local registration connects; project .mcp.json awaits approval
codex: registration recorded; `codex mcp` never connects (live call not run)
```
