---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8b80
title: 'Bug: Context menu shows raw template placeholders (e.g. "Delete {{entity.type}}")'
---
## What
Reported by user: right-click context menus render raw, un-interpolated template names like **"Delete {{entity.type}}"** instead of "Delete Task". The template name is not being rendered against the command context.

(Original 2026-06-05 analysis referenced `scope_commands.rs` / `resolve_name_template` / `list_commands_for_scope` — see Reconciliation below; those references are stale.)

## Reconciliation (2026-06-10) — stale references vs. current mechanism

The original analysis predates the caption mechanism shipped by card 01KTRMXRNH66GZCWSNR1YGE28E. Current state:

- The `scope_commands.rs` / `resolve_name_template` / `list_commands_for_scope` references are STALE for this bug's surface. The live render path is the command service: `handle_list` (`crates/swissarmyhammer-command-service/src/service.rs`) renders every listed command's `name` and `menu_name` through `render_caption(ctx)` (`crates/swissarmyhammer-command-service/src/caption.rs`). Entity type resolves from the explicit `ctx.target` moniker when present (context-menu semantics), else the innermost scope-chain moniker (palette semantics); no context → clean generic fallback ("Delete", never "Delete {{entity.type}}").
- The raw-placeholder SYMPTOM was therefore already gone (the server never emits `{{`). The original root-cause hypotheses were obsolete.
- REMAINING WORK (this card): the context-menu open path listed with NO ctx, so it got generic captions. Pass the clicked entity's context — `ctx.target` (innermost moniker) + `ctx.scope_chain` — to `list command` at right-click time so a task shows "Delete Task", a tag "Delete Tag", etc. Pure frontend wiring — ZERO template logic in React; captions arrive display-ready.

## Implementation (2026-06-10)

`useContextMenu` (`apps/kanban-app/ui/src/lib/context-menu.ts`) no longer reads a render-time `useCommandList()` snapshot. At right-click time it now fetches `list command` via `callCommandTool` with the click point's context — `ctx: { target: scopeChain[0], scope_chain: scopeChain }`, the same `ctx` wire shape the palette sends (`buildListParams`) plus the explicit context-menu `target` — then filters (`context_menu: true` + scope match), sorts into groups, and pops the native menu. Empty chain → no ctx → server generic fallback. Captions arrive display-ready; zero template logic in React. Side benefit: each right-click sees the current registry by construction, and the hook no longer runs a per-instance fetch/subscription at high-multiplier render sites (per grid cell/row).

Support changes: `ListCommandResult` exported from `use-command-list.ts`; shared test helper `answerListCommand` added to `src/test/mock-command-list.ts`; test seams migrated from the `useCommandList` module mock to the click-time `list command` transport mock in context-menu.test.tsx, context-menu.scoped.test.tsx, perspective-tab-bar.context-menu.test.tsx, focus-scope.test.tsx, attachment-display.test.tsx, left-nav.browser.test.tsx (the two left-nav right-click tests also needed a real async settle instead of two bare microtask flushes).

## Acceptance Criteria
- [x] Context-menu entries show fully-resolved names (e.g. "Delete Task", "Copy Tag") — no `{{…}}` ever reaches the UI (server renders against ctx.target; never-raw guaranteed by command-service guard sweep).
- [x] Root cause identified: pre-caption-mechanism the list path never rendered templates; the remaining gap was the context menu not passing ctx (see Reconciliation).

## Tests
- [x] Failing vitest first: new "click-time ctx caption rendering" describe in `context-menu.test.tsx` (4 tests: ctx wire shape, task → "Delete Task", tag → "Delete Tag", empty chain omits ctx). Watched RED (4 failed, `callCommandTool` never called) before the implementation; GREEN after. Additional red probe: stripping the ctx param alone re-fails exactly the 3 ctx-dependent tests, restored → green.
- [x] Per-entity correctness across two entity types (task vs tag) — server-simulating mock keyed off `ctx.target`.
- [x] Rust-side guard tests already exist (caption.rs unit tests, list_renders_captions.rs integration, full_baseline_e2e never-raw sweep) — no Rust changes made.
- [x] Scoped vitest green: context-menu.test.tsx (16), context-menu.scoped.test.tsx, left-nav.browser.test.tsx, perspective-tab-bar.context-menu.test.tsx, data-table.test.tsx, use-command-list.test.tsx — 46/46. `tsc --noEmit` clean.
- [x] Pre-existing (verified at HEAD baseline, identical set before/after): focus-scope.test.tsx 9 browser-mode failures + attachment-display 1 — filed as card 01KTS1C4EX8W6GZYPAYB1T431K, unrelated to this change.

## Workflow
- Used `/tdd` — failing test first, then fix. #bug

## Review Findings (2026-06-10 10:19)

### Warnings
- [x] `apps/kanban-app/ui/src/lib/context-menu.ts` (`openContextMenu` / the `useCallback` handler) — No supersede guard on the in-flight click-time `list command` fetch. The old path popped the menu synchronously from a snapshot; the new path opens an async window per right-click, and concurrent `openContextMenu` calls race: if the user right-clicks entity A, then entity B before A's fetch resolves, the two `show context menu` calls land in *resolution* order, so A's stale menu can pop last — with items whose `target`/`scope_chain` point at A while the user believes they're acting on B (wrong-entity dispatch, not just cosmetic). The codebase already treats stale list resolves as worth guarding: `use-command-list.ts` uses `fetchIdRef` for exactly this. Fix: keep a monotonically increasing click token in a ref (or module scope), capture it before the fetch, and bail before `callMcpTool("window", "show context menu", …)` if a newer click has superseded it (~5 lines).

### Resolution (2026-06-10)

Supersede guard added to `openContextMenu` (`apps/kanban-app/ui/src/lib/context-menu.ts`): module-scope monotonic `openId` token, captured as `myId = ++openId` at the top of the call, checked right after the `list command` await — `if (myId !== openId) return;` — so a stale click's response never reaches `show context menu`. Module scope (not a per-hook ref) is deliberate: rapid right-clicks can come from different hook instances (different components), and the native menu is one global resource — only the newest click may pop it. Same pattern as `use-command-list.ts`'s `fetchIdRef`.

TDD red-green: new test "drops the stale first click's menu when its fetch resolves after a newer click" in `context-menu.test.tsx` (new "supersede guard" describe) uses deferred-promise control over the `list command` mock — two opens (task:A then task:B), B's fetch resolved first, A's (stale) last. RED on pre-guard code (2 `show context menu` calls, stale A's last — `expected 2 to be 1`); GREEN after the guard (exactly 1 show, target `task:B`, caption "Delete B").

Verification: context-menu.test.tsx + context-menu.scoped.test.tsx 19/19; perspective-tab-bar.context-menu.test.tsx + left-nav.browser.test.tsx + data-table.test.tsx 20/20; `tsc --noEmit` exit 0.