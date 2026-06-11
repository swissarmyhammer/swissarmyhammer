---
assignees:
- claude-code
depends_on:
- 01KT9FY7SBW0MVVAZ4A1WZP4SS
- 01KT9FYTVE2CMAGZQW29G1M6Q6
- 01KT9FZ8GZWSPJTK04NEGC0WXQ
position_column: todo
position_ordinal: b180
project: command-cutover
title: Gate ai.cancel in the palette via event-driven cached availability (needs SDK subscribe API)
---
DISCOVERED reviewing 01KT6WWYYWFQ2F4PGQ358SAHY7 (ai.yaml → ai-commands plugin migration).

The `ai.cancel` ("Stop AI Generation") command registered by `builtin/plugins/ai-commands/index.ts` carries NO `available` callback, so the registry-driven palette (`useCommandList`/`useCommandAvailability` → backend `available command`) shows it as ENABLED even when no AI generation is in flight.

## Why not fixed now
- The command-service contracts `available` as SYNCHRONOUS (`ideas/plugins/command-service.md`: "The service contracts `available` as synchronous, returning `boolean | { ok: false, reason: string }`").
- The streaming flag (`status === "streaming"`) lives webview-side in `apps/kanban-app/ui/src/ai/commands.ts`'s module bus. The plugin isolate has NO synchronous handle to it, and `CommandContext` (`scope_chain` / `target` / `args`) carries no streaming flag.
- The correct fix is the event-driven cached-flag pattern (command-service.md: "the plugin subscribes to whatever changes the precondition, maintains a cached flag, returns it synchronously"). That needs the SDK event/subscription API (`on`/`subscribe`), which is currently INERT/RESERVED — `crates/swissarmyhammer-plugin/src/sdk/plugin.ts`'s `reservedHandler()` returns a no-op ("event API not implemented in this SDK task").

## Current state (acceptable interim — updated 2026-06-11 after Card I, 01KTED9JYGWM815K2X41N4QDBY)
- Card I deleted `app-shell.tsx`'s `buildAiCommands(streaming)` and its `available: streaming` CommandDef gate — there is no ai.* `CommandDef` in any React scope anymore. The five `ai.*` ids are DEFINED solely by the `ai-commands` plugin registration and EXECUTED through webview command-bus handlers that `AppShell` registers (`useAiCommandBusHandlers` in `apps/kanban-app/ui/src/components/app-shell.tsx`).
- The authoritative frontend gate is now DISPATCH-TIME: the `ai.cancel` bus handler reads `aiStreaming()` (`apps/kanban-app/ui/src/ai/commands.ts`) when dispatched and no-ops while idle — behaviorally equivalent to the old `available: false` (keybinding and palette dispatch both funnel through it). Covered by `app-shell.ai-commands.test.tsx` (idle no-op / streaming cancels / gate re-closes).
- What is still MISSING is unchanged: the *registry* palette listing has no availability metadata, so "Stop AI Generation" renders enabled while idle (dispatching it is a safe no-op).

## Work (once the SDK subscribe API lands)
- In `ai-commands` plugin: subscribe to the streaming-status change, cache the flag, and add a synchronous `available: () => cachedStreaming || { ok: false, reason: "No AI generation is running" }` to the `ai.cancel` registration.
- Wire the webview streaming status to the plugin via the new event surface (replacing the now-dead `ai_set_streaming` Tauri command + `UIState.ai_streaming` plumbing in `apps/kanban-app/src/ai/models.rs` / `crates/swissarmyhammer-ui-state/src/state.rs`, which no backend `available` reads anymore).

## Acceptance
- Registry-driven palette ("Stop AI Generation") is disabled/hidden when idle, enabled mid-stream, matching the frontend dispatch-time gate in `useAiCommandBusHandlers`.
- Depends on: SDK event/subscription API (`on`/`subscribe`) being implemented (currently RESERVED no-op).