# Diagnostics: Check-on-Save for Agents

## Concept

An editor syncs the buffer on change and surfaces diagnostics on save. An agent
should work the same way. The trigger is **the file change, not the tool call** — so
one service serves the local ACP agent, Claude Code, and a human editor alike, since
all three end in a write. Structured diagnostics are the output; delivery varies by
host.

## One LSP system (the invariant)

`swissarmyhammer-code-context` already spawns rust-analyzer (`lsp_server.rs`), runs a
full `async-lsp` client, and has its own `get_diagnostics`. A fresh diagnostics client
would be a second client, a second analyzer over the same tree, and a second
diagnostics implementation. Forbidden.

**One client session per (workspace, server), owned in one place, one shared
open-document set, with a notification fan-out. code-context and diagnostics are
sibling consumers; neither spawns servers or owns a client.**

The client is foundational, so it belongs in `swissarmyhammer-lsp`, promoted from
process supervisor to the one LSP system:

```
swissarmyhammer-lsp = supervision
                    + ONE async-lsp session per (workspace, server)
                    + ONE shared open-document set (didOpen/didChange/didSave)
                    + publishDiagnostics fan-out
                    + request API (references, callHierarchy, hover, …)

consumers:  code-context → symbol / call-graph / hover requests
            diagnostics  → subscribe to diagnostics, add settle + report shape
```

The client in code-context's `lsp_server.rs` moves down into `swissarmyhammer-lsp`;
code-context keeps its query ops but issues them through the shared client; its
`get_diagnostics` collapses into the one diagnostics path. The shared open-document
set is *why* this must unify rather than "reuse" — two clients each doing `didOpen`
give rust-analyzer two inconsistent views (or two analyzers).

The diagnostics core is thin: subscribe (push `publishDiagnostics`, or pull
`textDocument/diagnostic`), debounce to settle, map `lsp_types::Diagnostic` to report
types, cache. The cache is derived state — never persisted.

## Cross-process: one leader per working directory

The one-client invariant holds per process; across processes it needs arbitration, or
N SAH processes in the same repo (subagents, a CLI run, the editor's SAH) spawn N
rust-analyzers over the same tree. `swissarmyhammer-leader-election` already fits:
flock-based, keyed by workspace hash, with a leader `socket_path()` and re-election
(`try_promote`, `peek_leader_pid`).

Key the election on the canonical workspace root. LSP servers (rust-analyzer,
sourcekit-lsp, clangd) speak **stdio only** — one client, no listener — so only the
leader ever touches the server: it spawns the stdio child, does the single
`initialize`, and owns the one open-document set and id space. The leader fronts the
election socket with the **SAH request API** (`diagnose(paths)`, references, hover —
*not* raw LSP) and multiplexes follower calls onto its single stdio session,
demultiplexing responses and fanning out `publishDiagnostics`.

Followers must use the request API, not raw LSP passthrough: N followers each doing
`initialize` against a single-client server would break. That API already exists —
the diagnostics core's `diagnose` and code-context's query ops — so the socket just
carries it across the process boundary. This is transport-uniform: identical whether
the server is stdio or natively socketed (gopls), since the leader is the sole client
either way.

`LspDaemon`'s stdio child is unchanged. The new work is: the leader serving the
request API over `socket_path()` with a multiplexer onto the one session, and
followers becoming socket clients instead of constructing their own `LspDaemon`.

**Subagents:** in-process subagents share the parent's client directly; out-of-process
subagents connect to the leader's socket like any other follower.

The leader's watcher keeps the shared view consistent across processes — a follower's
direct `files edit` write is caught by the watcher, which issues `didChange`, so
followers need not sync the analyzer themselves. One watcher per workdir.

**Handoff:** if the leader exits, `try_promote` elects a follower, which re-spawns and
re-indexes rust-analyzer — a cold start, acceptable since handoff only happens when
the owning process exits. Surviving handoff without re-index needs a detached daemon;
defer until handoff frequency justifies it.

## Operation surface (pull + introspection)

The pull side is an operation tool, mirroring `review`. MCP tool `diagnostics`:

- `check working` — files changed vs HEAD (everyday op)
- `check file` — explicit path or glob
- `check sha` — files touched in/since a commit or range
- `list servers` / `get server` — read the supervisor, no analysis

Each `check` returns `DiagnosticsReport { diagnostics, counts }` with modifiers
`severity?`, `settle_ms?`, `dependents?`. (`severity` is the enum param that wants
`allowed_values`.) Defining these as operations means they flow through the same
dispatch, schema, and grammar as every other op — no bespoke schema or grammar.

The reactive paths below are not operations — they ride the same service.

## Diagnostics in the write op's result (no hook)

When a write op mutates a file, it returns diagnostics in its own result, the same
way it already returns the path and byte count. No seam, no config; the model sees
them because they're the return value of its call, in every host.

No per-op duplication: a write op declares `mutated_paths`; a single shared step in
the execution path reads that field and folds in diagnostics. Any future mutator gets
this the moment it reports a mutated path.

**When appropriate** — the shared step gates on:
- **Diagnosable language** (the supervisor knows). A `.md`/`.txt` edit attaches
  nothing.
- **Settle, generously.** Wait for real quiescence; a few seconds in-tool beats an
  extra model turn. `pending` is a backstop for pathological analysis only.
- **Severity/scope policy.** Edited file always; one-hop broken dependents folded in
  inline (blast radius is cheap, the turn it saves is not), ranked and capped.

## Design stance: do more per call

A local-LLM turn (full re-decode + generation, seconds, growing with context) is the
expensive unit; in-tool work is milliseconds to seconds. So a tool call should make
the model's next decision correct and final — every retry designed out is a turn not
paid for. Guardrail (k=1): do more *work*, keep *output* sharp; bloated results cost
context now and decode time next. Compute the full blast radius; return only what
broke.

## What drives the proactive check

Detection is uniform — the watcher is a continuous task in the persistent server
process: file change → debounce → LSP client → diagnostics. Whether the result
reaches the *model* depends on who owns the loop (MCP can't make a foreign model act
out of turn):

- **Own the loop (llama-agent).** The write op's result feeds straight back; also
  forwarded as an ACP `session/update` to the editor gutter. Complete.
- **Foreign host, edit via your tool.** Same mechanism — diagnostics are in the op
  result regardless of host. Model-facing.
- **Foreign host, native edit.** The ceiling. Your tool never runs; the watcher still
  detects and can emit `notifications/message`, but that's host/human-facing and
  can't wake an idle model.

Soft levers for the last case: make the pull habitual via tool description; expose
diagnostics as a subscribable MCP resource (`notifications/resources/updated`).

## Closing the write surface (shorting out native edits)

The hard lever that promotes the native-edit case to model-facing: force every
mutation through the instrumented tools. In Claude Code, permission `deny` on
`Edit` / `Write` / `MultiEdit` plus a `PreToolUse` hook that redirects to `files
edit`. This is host config the user installs (an MCP server can't disable a host's
built-in tools); swissarmyhammer ships the fragment. Hook-capable hosts only.

Editing can't be closed without shell already closed — an open `Bash` writes files
via `cat >`/`sed -i`, bypassing the tool and the diagnostics. So shell shorting is the
prerequisite; the goal is a **closed write surface**: no uninstrumented path by which
a file can change. The watcher remains the async backstop for what still leaks
(subprocesses, formatters, `git checkout`).

Tradeoff: native `Edit` is fast and the model is tuned to it; routing through MCP adds
latency and makes you own edit reliability. Worth it only while `files edit` stays at
least as reliable as the tool it displaces.

## Delivery surfaces

- **Tool result** — `check working` / `check file` return the structured report.
  Works in every host. The floor.
- **Inline on edit** — the write op's own result (above). The primary model-facing
  channel; bounded by settle, `pending` on timeout.
- **Push notification** — watcher → `notifications/message`. Host-facing; whether it
  renders as UI is up to the host. A courtesy, not load-bearing.
- **ACP session update** — llama-agent injects into model context *and* forwards to
  the editor's native gutter (e.g. Xcode).

## Debounce / settle

Language servers re-flow diagnostics as they analyze. Wait for a quiescence window
before reporting; never report mid-analysis state.

## Report sharply

Output side of "do more per call": compute freely, surface selectively. Always the
edited file; of its dependents, only those that *actually broke*. No project-wide
dump, no unrelated standing warnings — they cost context now and decode time next.

## Configuration

Severities, settle window, per-report cap, per-language enable. Defaults: errors +
warnings, short settle, capped, all detected languages. No persistence. The
`severity?` / `settle_ms?` / `dependents?` op params override per call.

## Crate

`swissarmyhammer-diagnostics` — settle/debounce, report types, config — on the shared
client in `swissarmyhammer-lsp` (owns no client of its own). It's a crate because it
has two consumers, the `diagnostics` tool and the `files` edit op, and belongs to
neither. Layout and dependency directions are in the file-edit-tools doc.

The tool and inline-on-edit ship in `swissarmyhammer-tools`; watcher-push and ACP
forwarding are llama-agent extras.

## Testing

- **Mapping**: `lsp_types::Diagnostic` → report record. Model-free.
- **Settle**: scripted revision stream → only the settled set; timeout → `pending`.
- **Integration**: rust-analyzer on a fixture crate with a known error; assert the
  report and the inline-on-edit attachment. Gated on the binary being present.
