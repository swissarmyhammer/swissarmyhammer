# Claude-Code Hooks (llama agent)

The llama ACP agent reads Claude Code's `.claude/settings.json` hook
configuration and fires hooks at the matching points in a session's lifecycle.
This lets an existing `.claude/settings.json` drive policy, context injection,
and tool gating for the llama agent without any agent-specific format.

The wiring is hermetic to each ACP session: hooks are loaded once per session
from that session's working directory, and a session whose cwd has no `.claude`
settings runs exactly as before, paying only one cheap settings-chain read at
session start.

## Events fired vs. accepted-but-never-fired

The llama agent fires the subset of Claude Code hook events that map to an ACP
lifecycle seam. Every other Claude Code event kind is **accepted** in the
settings file (so the same file works with both Claude Code and the llama
agent) but **never fires**. Such a kind is either dropped when registrations are
built or registered but never emitted — see the two categories below.

### Fired

| Event | Seam | Can block? |
|-------|------|-----------|
| `SessionStart` | `new_session` (source `startup`), `load_session`/`resume` (source `resume`) | No |
| `UserPromptSubmit` | Entry of a `prompt` turn | Yes (exit 2 / `decision: block`) |
| `PreToolUse` | Before each tool dispatch | Yes (`deny` / `continue: false`) |
| `PostToolUse` | After a successful tool call | No (context only) |
| `PostToolUseFailure` | After a failed tool call | No (context only) |
| `Stop` | Return of a `prompt` turn | "Block" ⇒ continue (see below) |
| `Notification` | Agent message / thought / plan notifications | No |

### Accepted but never fired (forward-compatible)

These event kinds deserialize without error and may appear in a shared
`.claude/settings.json` (so the same file works with both Claude Code and the
llama agent), but the llama agent never fires them. There are two distinct
reasons, and they behave differently at registration-build time:

**Dropped at registration build (no ACP `HookEventKind`).** Their
`HookEventKindConfig → HookEventKind` conversion returns
`Err(UnsupportedEventKind)`, so `build_registrations` skips them entirely — no
registration is ever created:

`PermissionRequest`, `SubagentStart`, `SubagentStop`, `PreCompact`, `Setup`,
`SessionEnd`.

**Registered but never emitted (no seam constructs the event).** These convert
to a valid `HookEventKind`, so registrations *are* built for them — but no
production seam ever constructs the corresponding `HookEvent` (the variants
exist only for tests and forward compatibility), so a matching hook never runs:

`PostCompact`, `TeammateIdle`, `TaskCompleted`, `Elicitation`,
`ElicitationResult`, `InstructionsLoaded`, `ConfigChange`, `WorktreeCreate`,
`WorktreeRemove`.

The net behavior is the same for both groups — the hook never fires — but only
the first group is dropped when registrations are built.

A malformed or unknown event kind never fails the agent — it is logged and the
rest of the config still loads.

## Settings file locations and precedence

Hooks are read from a three-file chain, resolved relative to the **session's
working directory** (the ACP session cwd is the project/workspace directory —
there is no ancestor walk-up). The chain, lowest precedence first:

1. **User** — `~/.claude/settings.json`
2. **Project** — `<cwd>/.claude/settings.json`
3. **Local** — `<cwd>/.claude/settings.local.json`

### Merge order is additive, not override

Claude Code runs *every* matching hook from *every* source. The loader mirrors
this: for each event name, the matcher groups from all three files are
concatenated in chain order (user → project → local). A `PreToolUse` group in
the user file and a `PreToolUse` group in the project file **both** fire — there
is no override.

### `disableAllHooks` is a hard off-switch

If `disableAllHooks: true` appears in *any* applicable file, the entire merged
config is dropped and the session runs with no hooks — even hooks declared in
the same file or a higher-precedence file. It is a global kill switch, not a
per-file opt-out.

### Not supported

Only the three `settings.json` files above are read. The following Claude Code
hook sources are **not** supported by the llama agent:

- **Plugin hooks** (`hooks/hooks.json` inside an installed plugin).
- **Managed-policy settings** (enterprise/managed `managed-settings.json`).
- **Skill / agent frontmatter hooks.**

Only the top-level `hooks` key of each file is read; every other key
(`permissions`, `env`, `statusLine`, `model`, …) is ignored.

## Handler types

A hook entry's `type` selects how its decision is produced. All three Claude
Code handler types are supported:

| `type` | Behavior |
|--------|----------|
| `command` | Run a shell command. Decision comes from the exit code and JSON stdout (see below). Default timeout 600s. |
| `prompt` | Single-turn LLM evaluation. The hook input JSON is rendered into the prompt (via `$ARGUMENTS`) and the llama model returns an allow/block decision. Default timeout 30s. |
| `agent` | Multi-turn agentic evaluation with tool access, backed by the same llama model. Default timeout 60s. |

`prompt` and `agent` handlers are evaluated by the per-session llama model
evaluator, so they reuse the loaded model — no separate model is configured for
hooks. A `prompt`/`agent` handler returns `{ "ok": true }` to allow or
`{ "ok": false, "reason": "..." }` to block.

## Tool-name mapping divergence

This is the most important difference from Claude Code. `PreToolUse` /
`PostToolUse` matchers are tested against the **llama-agent tool name**, the
bare name the model emits — *not* Claude Code's tool names.

Write matchers against the llama names, **not** `Bash` / `Edit` / `Write`:

| Claude Code name | llama-agent name |
|------------------|------------------|
| `Bash` | `shell` |
| `Read` | `fs_read` |
| `Write` / `Edit` | `fs_write` |
| `mcp__<server>__<tool>` | `mcp__<server>__<tool>` (same) |

### Canonical llama-agent tool-name list

The matcher value is the bare tool name the model emitted (never a decorated
display title). A matcher is regex-tested against that exact emitted name, so it
matches *only* the name the model actually used — not every alias that resolves
to the same capability. The canonical built-in names are:

- `shell` — execute a shell command. The model may emit either `shell` or
  `terminal`; both resolve to the terminal capability, but a matcher must target
  whichever name your model emits (`shell` does not also match a tool emitted as
  `terminal`).
- `fs_read` — read a file. The model may emit `fs_read`, `fs/read`, `read_file`,
  or `read_text_file`; all resolve to the read capability, but match whichever
  name your model emits (a `fs_read` matcher does not also match `read_file`).
- `fs_write` — write a file. The model may emit `fs_write`, `fs/write`,
  `write_file`, or `write_text_file`; all resolve to the write capability, but
  match whichever name your model emits.
- `mcp__<server>__<tool>` — any MCP tool, namespaced by its server. For example
  `mcp__sah__kanban`, `mcp__sah__code_context`. The `<server>` and `<tool>`
  segments are the MCP server name and the tool name it exposes.

A matcher is a regex (Claude Code semantics): `fs_write` matches exactly that
tool, `mcp__sah__.*` matches every tool from the `sah` MCP server, and an
omitted matcher matches every tool for that event. To match more than one alias,
use an alternation — e.g. `shell|terminal` or `fs_read|read_file`.

## Exit-code and JSON-stdout contract

`command` hooks use Claude Code's exit-code + JSON-stdout contract verbatim.

### Exit code

| Exit code | Meaning |
|-----------|---------|
| `0` | Success. stdout is parsed as the JSON contract (below); if it is not JSON, the hook simply allows. |
| `2` | Block. stderr is the reason. For `UserPromptSubmit` this blocks the prompt; for `PreToolUse` this denies the tool. |
| other | Non-blocking error; the hook is treated as allow and the stderr is surfaced as context. |

### JSON stdout

On exit 0, stdout may be a JSON object controlling the decision. Field names are
camelCase, matching Claude Code:

```json
{
  "continue": true,
  "stopReason": "shown to the user when continue is false",
  "suppressOutput": false,
  "systemMessage": "warning shown to the user",
  "decision": "block",
  "reason": "why the action was blocked",
  "additionalContext": "text appended to the model's context",
  "hookSpecificOutput": { "hookEventName": "PreToolUse", "...": "..." }
}
```

- `continue: false` stops the turn entirely (takes precedence over everything
  else). At the tool seam this maps to "stop the turn without dispatching."
- `decision: "block"` blocks the action; `reason` is the message. For `Stop`,
  `decision: "block"` means "do not stop" (see ACP notes).
- `additionalContext` is appended to what the model sees next — used by
  `UserPromptSubmit`, `PostToolUse`, `PostToolUseFailure`, and `SessionStart`.
- `hookSpecificOutput` carries the per-event fields, tagged by `hookEventName`:
  - `PreToolUse` → `permissionDecision` (`allow` / `deny` / `ask`),
    `permissionDecisionReason`, `updatedInput`, `additionalContext`.
  - `PostToolUse` / `PostToolUseFailure` / `UserPromptSubmit` / `SessionStart` /
    `Notification` → `additionalContext`.
  - `Stop` → `reason`.

### Command hook stdin

Each `command` hook receives the event as JSON on **stdin**, including
`hook_event_name`, the event's fields (e.g. `tool_name`, `tool_input`, `cwd`,
`source`), the session's `transcript_path` (the per-session `raw.jsonl`, so a
hook can read the transcript), and the `permission_mode` string.

The `permission_mode` is mapped from the llama agent's coarser permission policy:
`AlwaysAsk` → `default`, and `AutoApproveReads` / `RuleBased` → `acceptEdits`.
It is informational; it does not gate firing.

## ACP-specific notes

- **PreToolUse blocks at the real dispatch seam.** Tool hooks fire
  synchronously around the actual tool dispatch, so a `deny` (or
  `permissionDecision: deny`) genuinely prevents the tool from running. The
  model receives the deny reason as the tool's result and the turn continues —
  matching Claude Code's "blocked" behavior. An `updatedInput` rewrites the tool
  arguments *before* dispatch; `additionalContext` is appended to the result fed
  back to the model. PreToolUse fires exactly once per tool call.
- **Stop ⇒ continue.** A `Stop` hook that "blocks" (`decision: block`) means
  "don't stop yet." The finished turn's response is annotated with
  `hook_should_continue: true` and `hook_reason` on its meta so the client can
  observe the request to keep going.
- **`Notification` is the only notification-family hook.** Agent message,
  thought, and plan notifications fire the `Notification` event. The
  notifications themselves are still broadcast to the client UI regardless of
  hooks; only the hook firing is gated.
- **`SessionStart` source.** `new_session` fires `SessionStart` with source
  `startup`; `load_session` / `resume` fires it with source `resume`. It is
  idempotent per session id — a resume after a load does not re-fire startup.

## End-to-end behavior summary

For a session whose cwd carries a real `.claude/settings.json` (and/or
`settings.local.json`), and a hermetic user-level `~/.claude/settings.json`:

- `SessionStart` fires once on `new_session`.
- A `UserPromptSubmit` command hook exiting 2 blocks a forbidden prompt before
  the model is ever invoked; the reason reaches the client.
- A `PreToolUse` `deny` prevents the matched tool from dispatching; the model
  sees the reason as the tool result.
- `PostToolUse` `additionalContext` reaches the model after a successful call.
- A `Stop` hook that blocks sets `hook_should_continue` on the response meta.
- `disableAllHooks: true` disables all of the above end-to-end.
- A user-level (HOME) hook and a project hook for the same event both fire
  (additive precedence).
