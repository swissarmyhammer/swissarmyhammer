# Closing the Write Surface

SwissArmyHammer attaches live LSP diagnostics to every file mutation, so the
model sees what its edit broke *on the same turn* it makes the edit. That only
works if the mutation actually flows through the instrumented `files` MCP tool
(`op: "edit file"` / `op: "write file"`), which folds the diagnostics in. A
mutation that goes around the tool produces no diagnostics — it is invisible to
the inline fold-in.

The **goal** is a *closed write surface*: every byte written to the working tree
goes through the instrumented tool, so diagnostics always ride the result. This
page describes how SwissArmyHammer closes the editing half of that surface on
Claude Code, the prerequisite it still depends on, and the tradeoff it accepts.

## An MCP server can't disable a host's native tools

Claude Code ships its own native `Edit`, `Write`, and `MultiEdit` tools. An MCP
server (which is what `sah serve` is) can *add* tools, but it cannot *remove* or
disable the host's built-in ones. As long as the native mutators are present and
allowed, the model — tuned to reach for them — will, and those edits bypass the
instrumented path.

So the editing surface is closed not from inside the server but with a **host
config fragment**: a Claude Code `settings.json` change that

1. sets `permissions.deny` on `Edit`, `Write`, and `MultiEdit`, so the model is
   told not to use the native mutators, and
2. adds a `PreToolUse` hook on those same tools that, if one is attempted
   anyway, denies it and redirects the model to the `files` MCP tool's
   edit/write op.

This fragment is **installed for you** — it is shipped through the same
`sah init` config surface that registers the MCP server and writes the
statusline, not something you hand-author. It is plain, Claude-shaped
`settings.json`: valid on every Claude Code version, inert on agents that don't
read those keys, and a no-op on hosts that have no hook support (an unrecognized
`hooks` block is simply ignored rather than an error).

## Prerequisite: the shell must be closed first

Closing the **editing** tools is not enough to close the **write surface**. An
open `Bash` tool can write files directly — `cat > file`, `sed -i`, `tee`,
redirection — entirely outside any edit tool and therefore outside the
diagnostics fold-in. While a general-purpose shell is available, the write
surface has a hole no edit-tool deny can patch.

So **shell-shorting is the prerequisite** for a truly closed write surface:
until the shell is constrained to a tool that cannot perform arbitrary file
writes, denying the edit tools narrows the gap but does not seal it. Closing the
shell is a separate initiative; this editing-surface fragment is one half of the
whole.

For everything that still leaks — through the shell today, or through any future
gap — the **leader watcher remains the async backstop**: a single leader-owned
file watcher per workdir notices changes on disk and re-flows diagnostics out of
band, so a bypassing write is caught eventually even though it did not ride an
edit-tool result inline.

## The tradeoff: latency and reliability

Routing edits through MCP is not free, and the choice is deliberate.

- **Native `Edit` is fast and the model is tuned to it.** It is an in-process
  tool call with no extra round-trip; the model reaches for it fluently.
- **Routing through the `files` MCP tool adds latency** — an extra hop to the
  server and back — **and makes us own edit reliability.** When the model edits
  through our tool, *our* tool's correctness (encoding preservation, line-ending
  preservation, atomic replacement, exact-match semantics) is what stands
  between the model and a corrupted file.

That cost is worth paying **only while `files edit` stays at least as reliable as
the native tool it displaces.** The whole point is to gain diagnostics on every
mutation; if the instrumented path were flakier than the tool it replaces, we
would be trading correctness for visibility, which is a bad trade. The bar for
keeping this fragment installed is that the redirect target never regresses below
the native tool's reliability.
