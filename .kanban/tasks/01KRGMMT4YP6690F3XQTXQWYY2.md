---
assignees:
- claude-code
depends_on:
- 01KRGMM6WNHA04KQVAMQATEASS
position_column: done
position_ordinal: ffffffffffffffffffffffffc780
project: rebuild-index
title: General-purpose CLI progress renderer for MCP notifications
---
Build a CLI-side progress consumer that any MCP-invoking CLI tool can wire in. The renderer must be **op-agnostic**: it consumes `ProgressNotificationParam` and renders progress; it has no special-case code for `rebuild index`. Future MCP ops that emit progress automatically get a TUI without changes.

This lives in `code-context-cli` (or, if it's broadly useful, in a shared crate like `swissarmyhammer-cli-progress` — decide during implementation based on whether `kanban-cli` / `shelltool-cli` also need it).

## Design

```rust
pub trait ProgressRenderer {
    fn on_notification(&mut self, n: &ProgressNotificationParam);
    fn finish(&mut self);
}

pub struct IndicatifRenderer { /* per-token ProgressBar map */ }

impl ProgressRenderer for IndicatifRenderer {
    fn on_notification(&mut self, n: &ProgressNotificationParam) {
        let bar = self.bars.entry(n.progress_token.clone()).or_insert_with(|| {
            let pb = ProgressBar::new(n.total.unwrap_or(0) as u64);
            pb.set_style(/* {msg} [{bar:40}] {pos}/{len} */);
            pb
        });
        if let Some(t) = n.total { bar.set_length(t as u64); }
        bar.set_position(n.progress as u64);
        if let Some(m) = &n.message { bar.set_message(m.clone()); }
    }
    fn finish(&mut self) {
        for bar in self.bars.values() { bar.finish(); }
    }
}
```

`indicatif` auto-degrades on non-TTY stdout to plain line output — no extra branch needed.

## Wiring

In `code-context-cli/src/commands/ops.rs`, the tool dispatch path needs to:

1. Generate a fresh `progressToken` (UUID or counter) and attach it to the outgoing request's `_meta`
2. Subscribe to the in-process notification channel (we're calling `CodeContextTool::execute` directly today — see if rmcp lets us tap notifications without going through stdio, or if we need to refactor the dispatch to thread a `NotificationSink`)
3. Drive a `ProgressRenderer` from each incoming notification on a tokio task
4. When the tool call completes, call `renderer.finish()`

The dispatch wrapper should look like:
```rust
async fn dispatch_with_progress<R: ProgressRenderer>(
    args: Map<String, Value>,
    mut renderer: R,
) -> Result<CallToolResult> { ... }
```

So future op handlers just call this — they don't need their own progress logic.

## Disabling

Add a `--no-progress` flag that swaps in a `NullRenderer`. Useful for CI / piped scripts where `indicatif`'s TTY detection might still get the wrong answer.

## Tests

- Unit: `IndicatifRenderer` with a fake bar (verify position/length/message updates per notification)
- E2E: spawn the CLI against a tmp workspace, invoke `rebuild index`, assert exit code 0 and that stdout/stderr contains the expected final summary line. Don't assert on the live bar updates (they're terminal control codes); just final output.

## Depends on

- MCP progress reporter card (this is the consumer of those notifications)

#cli #mcp #code-context #rebuild-index