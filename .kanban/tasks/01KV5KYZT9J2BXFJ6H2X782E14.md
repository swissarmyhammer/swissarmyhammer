---
assignees:
- claude-code
position_column: todo
position_ordinal: fe80
project: builtin-commands
title: 'Fix: "Inspect Board" from the toolbar context menu no-ops (chrome leaf passed as inspect target)'
---
## Problem

"Inspect Board" on the board scope in the toolbar (nav-bar) works from the command palette but does nothing from the **context menu**.

Root cause — a target/caption divergence:

- The context menu sets the dispatch `target` to the **innermost (leaf) scope-chain moniker**: `apps/kanban-app/ui/src/lib/context-menu.ts:141` → `target: scopeChain[0]`. In the toolbar the leaf is a **chrome** moniker (`ui:navbar.board-selector` / `ui:navbar.inspect` / `ui:navbar.search`), not `board:{id}` — the nav-bar deliberately mounts `ui:navbar.*` leaves (`apps/kanban-app/ui/src/components/nav-bar.tsx`), with `board:{id}` only as an ANCESTOR in the chain (from `board-container.tsx`).
- The inspect execute trusts that target verbatim: `resolveInspectTarget` (`builtin/plugins/app-shell-commands/commands/ui.ts:321`) returns `ctx.target` as-is when present. So it calls `ui_state inspect inspector` with `moniker: "ui:navbar.board-selector"` — a non-inspectable chrome moniker → the inspector no-ops.
- Meanwhile the menu LABEL "Inspect {{entity.type}}" → "Inspect Board" is rendered by `caption::focused_entity_type`, which **skips chrome** and resolves `board:` from the chain. So the caption promises "Inspect Board" while the execute inspects a chrome leaf — they disagree.
- The palette works because its dispatch carries no chrome `target`; `resolveInspectTarget` falls through to the innermost inspectable scope-chain moniker and finds the `board:{id}` ancestor. (The explicit toolbar inspect BUTTON also works — `nav-bar.tsx:110` passes `target: board.board.moniker`.)

## What

Make `resolveInspectTarget` agree with `focused_entity_type`/the caption: an explicit `ctx.target` should only win when it is an **inspectable-entity moniker**; otherwise fall through to the innermost inspectable scope-chain moniker. One-place fix in the inspect command's own resolver — not a per-surface patch.

- `builtin/plugins/app-shell-commands/commands/ui.ts` — change `resolveInspectTarget` so a `ctx.target` that is NOT one of `INSPECTABLE_ENTITY_PREFIXES` (`task:`/`tag:`/`column:`/`board:`/`attachment:`, ui.ts:266) is ignored, and resolution falls to `(ctx.scope_chain ?? []).find(isInspectable)`:

  ```ts
  const isInspectable = (m: string) =>
    INSPECTABLE_ENTITY_PREFIXES.some((p) => m.startsWith(p));
  function resolveInspectTarget(ctx: CommandContext): string | undefined {
    if (ctx.target !== undefined && isInspectable(ctx.target)) return ctx.target;
    return (ctx.scope_chain ?? []).find(isInspectable);
  }
  ```

  This mirrors `focused_entity_type` (chrome `ui:*` and `field:` projection monikers are skipped), so caption and executed inspect can never disagree. A non-inspectable explicit target was unusable anyway (the inspector no-ops on it), so falling through is strictly better.
- Do NOT change `context-menu.ts`'s generic `target: scopeChain[0]` (other commands legitimately want the leaf target) and do NOT special-case the nav-bar. The fix belongs in the inspect resolver.

## Acceptance Criteria
- [ ] Right-clicking the board scope in the toolbar and choosing "Inspect Board" opens the inspector on the board (dispatches `ui_state inspect inspector` with the `board:{id}` moniker), matching the palette behavior.
- [ ] A context-menu inspect whose leaf target IS an inspectable entity (e.g. right-click a task card → target `task:{id}`) still inspects that exact entity verbatim (no regression).
- [ ] Inspect from the palette and the explicit toolbar inspect button are unchanged.
- [ ] `entity.inspect` (Space) behavior is unchanged (it already resolves via the scope chain).

## Tests
- [ ] Production-path test in `crates/swissarmyhammer-command-service/tests/integration/builtin_app_shell_commands_e2e.rs` (loads the real `app-shell-commands` plugin in a V8 isolate and dispatches `app.inspect`): dispatch with `ctx = { target: "ui:navbar.board-selector", scope_chain: ["ui:navbar.board-selector", "board:b1", "window:main"] }` and assert the `ui_state inspect inspector` call receives `moniker: "board:b1"` (currently it receives the chrome moniker / no-ops). Add the inverse: `target: "task:t1"` with a task scope chain still inspects `task:t1`.
- [ ] If a TS-level harness exists for the inspect execute (e.g. alongside `apps/kanban-app/ui/src/test/inspectable-entity-prefixes.ts` / the ui-commands node tests), add a unit case for `resolveInspectTarget`/`buildInspectExecute`: chrome `target` + `board:` in `scope_chain` → resolves `board:`; inspectable `target` → verbatim.
- [ ] `cargo test -p swissarmyhammer-command-service --test integration` passes (new assertion red before the fix, green after).

## Workflow
- Use `/tdd` — write the chrome-target → board-scope-chain inspect assertion first (it fails: inspects the chrome moniker), then tighten `resolveInspectTarget` to make it pass without regressing the inspectable-target-verbatim case. #bug #commands #frontend