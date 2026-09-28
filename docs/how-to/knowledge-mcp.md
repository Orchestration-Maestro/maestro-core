# Connect a client to Maestro's knowledge MCP server

This guide covers Bash/WSL command forms for Pi with the `pi-mcp-adapter`,
GitHub Copilot CLI, Codex CLI, and Claude Code. The examples register
Maestro's local stdio server; they do not qualify a client/provider
combination or authorize sending collection data to a remote model. Check each
installed client and adapter's help/version before use. Grant tools explicitly;
do not assume the client's working directory selects Maestro preferences.
The registration examples below work with the current S1 server. The following
S3 workspace contract is planned, not a claim that its flag already exists.

## Prepare a local installation

Set these values in your shell, using paths from your machine (do not paste
literal placeholders into client configuration):

```sh
export MAESTRO_BIN="/path/to/maestro"
export MAESTRO_CONFIG_HOME="/path/to/config-home"
export MAESTRO_DATA_HOME="/path/to/data-home"
export MAESTRO_MCP_CONFIG="/path/to/private/maestro-mcp.json"
```

`MAESTRO_BIN` must be the absolute path to the executable. The two `*_HOME`
values are base directories: Maestro reads and writes their `maestro`
subdirectories. Configure an existing local collection binding in
`bindings.toml` and grant only the collection the user should read in
`config.toml`, for example:

```toml
[access]
read = ["workspace/default/collection/synthetic"]
```

The collection must already have a published generation. Registration does not
import, publish, or grant access. The server uses the local `local` principal;
a client cannot grant itself additional scope.

## S3 workspace preferences (planned)

A server implementing S3 uses user `preferences.toml` unless its registration
explicitly supplies `--workspace DIR`. Client cwd and MCP roots do not select
workspace settings in S3. Register one exact workspace when project overrides
are needed; user-scoped registrations do not automatically follow projects.

For Pi, replace the `args` array below with:

```json
["mcp", "--workspace", "/path/to/project"]
```

For Copilot, Codex and Claude Code, set `MAESTRO_WORKSPACE` to the actual
canonical project directory and replace the final server argv in the command:

```sh
-- "$MAESTRO_BIN" mcp --workspace "$MAESTRO_WORKSPACE"
```

Do not use these extra arguments until the installed server supports them.
After restarting, inspect initialization instructions: they say whether a
workspace was selected or user/default preferences are in use, without sending
absolute paths to the model. The registered directory uses the same safe
ownership/home/trust discovery as CLI. Outside home, approve the canonical
folder locally with `maestro trust add DIR` before workspace settings can load;
config files never grant trust. No --workspace means user preferences even if
Pi, Copilot, Codex or Claude Code starts the server inside a project. Roots-based
workspace detection is a named follow-up once a client-qualified channel exists.

## Register Maestro

### Pi (`pi-mcp-adapter`)

Install and approve the `pi-mcp-adapter` version for your Pi installation.
Create the private JSON file named by `MAESTRO_MCP_CONFIG`, replacing the
placeholders with the resolved values (JSON does not expand shell variables):

```json
{
  "mcpServers": {
    "maestro": {
      "command": "/path/to/maestro",
      "args": ["mcp"],
      "env": {
        "XDG_CONFIG_HOME": "/path/to/config-home",
        "XDG_DATA_HOME": "/path/to/data-home"
      }
    }
  }
}
```

Start Pi with the adapter's explicit configuration:

```sh
pi --mcp-config "$MAESTRO_MCP_CONFIG"
```

Use `/mcp setup` for the adapter's persistent setup preview if preferred.

### GitHub Copilot CLI

```sh
copilot mcp add maestro \
  --env "XDG_CONFIG_HOME=$MAESTRO_CONFIG_HOME" \
  --env "XDG_DATA_HOME=$MAESTRO_DATA_HOME" \
  --tools knowledge_collections,knowledge_search,knowledge_get,knowledge_ask \
  -- "$MAESTRO_BIN" mcp
```

### Codex CLI

```sh
codex mcp add maestro \
  --env "XDG_CONFIG_HOME=$MAESTRO_CONFIG_HOME" \
  --env "XDG_DATA_HOME=$MAESTRO_DATA_HOME" \
  -- "$MAESTRO_BIN" mcp
```

### Claude Code

```sh
claude mcp add --transport stdio --scope user maestro \
  --env "XDG_CONFIG_HOME=$MAESTRO_CONFIG_HOME" \
  --env "XDG_DATA_HOME=$MAESTRO_DATA_HOME" \
  -- "$MAESTRO_BIN" mcp
```

These commands add a user-scoped registration. Before replacing an existing
`maestro` entry, inspect and back it up; do not overwrite client configuration
or disable a client's tool or workspace security policy. If an enterprise
policy blocks registration, stop and request approval from its administrator.

## Verify and use the tools

Restart or reload the client, inspect its effective Maestro command and
environment, then discover the server tools. The server advertises
`knowledge_collections`, `knowledge_search`, `knowledge_get`, and
`knowledge_ask`. Select only the tools needed in the client permission prompt.
For example, call `knowledge_search` with:

```json
{"collection":"synthetic","query":"What does the glossary say?"}
```

A successful search returns `maestro-evidence/1`, including the selected
collection and published generation, cited passages, and route status. Keep
that provenance and any truncation/budget information when using a client that
summarizes tool output. Search budget defaults are `k: 10`, `max_tokens: 6000`,
and `deadline_ms: 1500`; accepted maxima are `k: 50`, `max_tokens: 12000`, and
`deadline_ms: 10000`. The token budget is an estimate unless the configured
route provides exact token counting. `knowledge_ask` is grounded in returned
evidence and can refuse when the evidence does not support an answer.

A local stdio process does not make a remote model's processing local. Before
using a remote provider, confirm that the provider/account and the collection
data scope are approved. Never treat a client's login, installed tool, or
retrieved content as authorization to send data or execute commands.

## If it does not connect

- **Executable or argv error:** verify `MAESTRO_BIN` is executable and the
  registration invokes `mcp`; use --workspace only with an S3-capable binary.
- **Wrong data or missing collection:** check both base directories, local
  bindings and the exact collection grant; confirm the collection has a
  published generation.
- **Tool approval or policy refusal:** inspect the client's effective user
  registration and request administrator approval; do not bypass policy.
- **Unavailable model route:** search may still return route status; ask may
  refuse. Restore or approve a configured route rather than enabling fallback.
