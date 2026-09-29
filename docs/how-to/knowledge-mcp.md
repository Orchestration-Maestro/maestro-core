# Connect a client to Maestro's knowledge MCP server

This guide covers Bash/WSL command forms for Pi with the `pi-mcp-adapter`,
GitHub Copilot CLI, Codex CLI, and Claude Code. The examples register
Maestro's local stdio server; they do not qualify a client/provider
combination or authorize sending collection data to a remote model. Check each
installed client and adapter's help/version before use. Run clients in a
private, empty working directory and grant tools explicitly.

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
and `deadline_ms: 30000` (a safety cap, not a quality cutoff: a search after
the router unloaded its models loads them again, which took up to 5 s each on a
busy machine, and still runs every route); accepted maxima are `k: 50`,
`max_tokens: 24000`, and `deadline_ms: 30000`. Despite its name, `max_tokens`
counts UTF-8 bytes of evidence: 6000 bytes is about 1,500 tokens of English
text, not 6,000. `knowledge_ask` is
grounded in returned evidence and can refuse when the evidence does not support
an answer. Its search has the same 30 s cap, and each answer attempt 20 s, the
answerer's load included. The server ends a search call after 40 s and an ask
call after 55 s, under the 60 s tool timeout common to MCP clients; a warm
search takes under 1.5 s and a warm ask under 10 s.

A local stdio process does not make a remote model's processing local. Before
using a remote provider, confirm that the provider/account and the collection
data scope are approved. Never treat a client's login, installed tool, or
retrieved content as authorization to send data or execute commands.

## If it does not connect

- **Executable or argv error:** verify `MAESTRO_BIN` is executable and the
  registration ends with `mcp`.
- **Wrong data or missing collection:** check both base directories, local
  bindings and the exact collection grant; confirm the collection has a
  published generation.
- **Tool approval or policy refusal:** inspect the client's effective user
  registration and request administrator approval; do not bypass policy.
- **Unavailable model route:** search may still return route status; ask may
  refuse. Restore or approve a configured route rather than enabling fallback.
