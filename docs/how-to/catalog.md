# Initialize a workspace

Run `maestro init --plain` inside the workspace. Select an explicitly reviewed
catalog directory and preset; neither has an implicit default. This is authoring
convenience, not a verified installation. No scripts or prerequisites execute.

The plain flow has five labelled stages:

1. Workspace, trust and preset: inspect the canonical root and existing approval.
2. Language: choose `en`, `fr`, `es` or another supported BCP 47 subset tag.
3. Tone: choose `brief`, `normal` or `detailed`.
4. All settings: inspect every S1 registry descriptor and enter `KEY=VALUE`.
5. Review: inspect exact proposed files, digests and collisions before deciding.

Enter keeps a language/tone choice or advances the settings editor. Type `back`
to return without losing choices, `cancel` to stop, or `preview` at review to
exit without writing. EOF and Ctrl-C cancel. Invalid input stays at its prompt.
Confirmation defaults to no. `--apply` is required even after an explicit yes.

Prompts go to stderr; the existing versioned plan is printed on stdout. No raw
mode, alternate screen, cursor repaint or color is used. `--no-color`, `NO_COLOR`
and `TERM=dumb` need no special terminal support. Unsupported interface languages
show the existing English-interface fallback note; conversational language stays
selected. Artifact and log language always stays English.

Trust is not a setting. A staged yes grants nothing on preview or cancellation.
After confirmed apply, an unapproved root needs the existing, separate default-no
trusted-terminal approval. Piped input and `--yes` cannot grant trust. Declining
trust reviews only the separately confirmed root-local preferences record, never
template or projection files. The same narrow review is shown on preview-only runs.

## Script the same plan

Use explicit flags and `--yes` to avoid prompts:

```sh
maestro init --catalog-dir "$CATALOG" --preset base --yes \
  --language fr --tone brief --updates off --set tool_calls=20
```

Without `--apply`, this previews only. Flags and plain choices share one
validator, planner and owned-file writer. Duplicate setting keys and invalid
values refuse. Updates persist as `off` when off, otherwise `propose`; init
cannot enable the user-only Auto ceiling. Omitted language resolves through
workspace/user defaults, finally `en`; tone finally defaults to `normal`.

In a fresh home, explicitly approve the exact canonical workspace first:

```sh
ROOT=$(pwd -P)
maestro trust add "$ROOT" --confirm-path "$ROOT"
maestro init --catalog-dir "$CATALOG" --preset base --yes --apply
maestro config explain
```

Missing approval prints the exact trust command and writes nothing. `--yes`
accepts choices, not trust or file collisions. Non-interactive apply without
`--yes` refuses. Missing catalog/preset selections require the plain flow.
`--json --yes` emits only the existing `maestro-cli/init/1` document, never a
menu or escape sequence. Successful apply reports `.maestro/config.toml` and
`maestro config explain` for inspecting effective values.

## Edit every setting

Run `maestro config` for the same registry-generated editor, defaulting to user
preferences. Run `maestro config --project` to use S1's existing project-file
selection, or `maestro config --user` for explicit user selection.

Each entry shows its current effective value, accepted values/range, one-line
description, source layer and editing restriction. Enter `KEY=VALUE`, press
Enter to review, and type `yes` to commit. Back keeps pending edits; cancel,
EOF and preview write no settings or journal entries. Confirmed edits use the
existing S1 config API and change journal, preserving its file-safety checks.

Locked settings cannot be edited; `maestro config explain KEY` describes them.
Standard-only descriptors require their central standard, validated by
`maestro catalog check --catalog-dir "$CATALOG"`. Authority is administered
with `maestro trust`, not a preference toggle. Workspace updates accept only
`off` or `propose`; only `maestro config set updates auto --user` may declare
Auto consent, still subject to all existing trust and narrowing restrictions.
For scripts or JSON, keep using `config list`, `get`, `set`, `unset` and `history`.
