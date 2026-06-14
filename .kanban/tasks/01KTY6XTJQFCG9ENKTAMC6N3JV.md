---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa380
title: Palette "Inspect Task" doesn't inspect; field focus yields "Inspect Field" instead of the containing task's inspect
---
## What

LIVE BUG (user-observed), two related symptoms in the command palette's inspect entry:

1. **"Inspect Task" in the palette doesn't actually inspect** — selecting it does nothing visible (no inspector opens).
2. **Wrong target when a field is focused**: with a field selected inside a task, the palette shows "Inspect Field". That makes no sense in context — the expectation is **"Inspect Task" resolving to the CONTAINING task**.

## Background (this is Card G's seam — read it first)

`entity.inspect` is a single GLOBAL plugin command (card 01KTED8MS8917AJCDAVHKSZHK7, then renamed `app.*` sweep): Space-bound, `visible:false`... (check current visibility — if visible:false, what palette entry is the user seeing? `app.inspect`? BOTH exist — `app.inspect` routes to ui_state inspect and `entity.inspect` resolves server-side; determine WHICH command the palette row is). Server-side `resolveInspectTarget` (builtin/plugins/app-shell-commands/commands/ui.ts): explicit `ctx.target` wins → else innermost inspectable-prefixed moniker from leaf-first `scope_chain` (prefixes: `task:/tag:/column:/board:/field:/attachment:` — pinned by INSPECTABLE_ENTITY_PREFIXES mirror tests) → else **inert `{ok:true}` no-op**.

Both symptoms plausibly stem from that resolution chain:
- **Symptom 1 (no-op inspect)**: the palette is an OVERLAY — by the time the user picks a row, the focused scope chain may be the PALETTE's own chain (or empty), not the entity's. Resolution finds no inspectable prefix → silent inert no-op (exactly "isn't actually inspecting"). Check what scope_chain the palette dispatch carries (does the palette capture the pre-open focus chain and dispatch with it, like context-menu carries an explicit chain — or does it dispatch with the live/post-open chain?). Also check the caption: the palette row said "Inspect Task" — captions render from the LIST-time ctx (the pre-open chain), so caption and execute-time resolution may disagree — caption promised Task, execute resolved nothing.
- **Symptom 2 (field wins)**: `field:` is in INSPECTABLE_ENTITY_PREFIXES, so a focused field inside a task resolves innermost-first to the FIELD. Design directive from the user: a field-of-a-task focus should resolve inspect to the CONTAINING TASK. Decide the clean rule: either (a) remove `field:` from the inspect prefix list entirely (check whether anything legitimately inspects a field — field-def entities in the fields grid may have their own different moniker shape; verify before removing), or (b) skip `field:` when an entity ancestor (task:/tag:/etc.) exists in the chain. Prefer the simplest data-driven rule; update the prefix mirror tests + Card G's e2e innermost-wins cases accordingly (they currently PIN field-inside-card → field — that pin must change to match the corrected semantics).

## Forensics FIRST

1. Reproduce via the log: open palette on a focused task, pick Inspect — `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' --last 30m | grep -iE 'inspect|dispatch_command|scope_chain'` — capture the dispatched cmd id + scope_chain + what resolveInspectTarget did.
2. Read the palette dispatch path (command-palette.tsx → useDispatchCommand): what chain rides the dispatch after the palette stole focus?
3. Determine which command id the palette row is (entity.inspect vs app.inspect) and which caption template it renders.

## RESOLUTION (implemented)

**Symptom 1 root cause (log-proven)**: the palette row is `app.inspect` ("Inspect {{entity.type}}", the only visible inspect; `entity.inspect` is visible:false). Its execute was `moniker: ctx.target ?? ""` — NO chain resolution at all. Log evidence (10:20:15 + 10:22:34, Jun 12): `cmd=app.inspect target=None scope_chain=["task:01KTC7GT...", ...]` → result `InspectorStack:[""]` — the chain DID carry the task; the execute ignored it and inspected the empty string (even pushing `""` onto the stack). Fixed: `app.inspect` and `entity.inspect` share ONE execute (`buildInspectExecute` → `resolveInspectTarget`); the no-target no-op is now warn-logged via the plugin Logger. The palette additionally captures the pre-open chain at open and uses it for list ctx + availability + dispatch (explicit `{scopeChain}`, the context-menu pattern) so caption and execution can never disagree.

**Symptom 2 rule chosen**: (a) — `field:` removed from INSPECTABLE_ENTITY_PREFIXES. Verified legitimate: field monikers are `field:{type}:{id}.{name}` projections explicitly designed "so field monikers don't masquerade as entity monikers in the scope chain" (moniker.ts), and kanban's `emit_scoped_commands` already skips them; fields stay inspectable via explicit target (double-click `<Inspectable>`), which wins verbatim. The caption renderer (caption.rs `focused_entity_type`) now filters the chain by the same list (Rust copy pinned 1:1 against the plugin source by `tests/inspectable_prefixes_mirror.rs`; webview mirror updated; Guard B/C extend with `field:` as explicit-target-only).

## Acceptance Criteria
- [x] Task focused → palette row "Inspect Task" → inspector opens for that task
- [x] Field-inside-task focused → palette row reads "Inspect Task" and inspects the containing task (no "Inspect Field" in this context)
- [x] Space-on-focused-field behavior matches the same corrected rule (containing task)
- [x] Caption and execute-time target NEVER disagree (same chain, same rule)
- [x] Resolution failure is warn-logged, not silent
- [x] Root cause documented (what chain the palette was dispatching with; log evidence)

## Tests
- [x] e2e (builtin_app_shell_commands_e2e or list_renders_captions): field-inside-task chain → caption "Inspect Task" AND resolveInspectTarget → the task moniker (update the existing innermost-wins pins that currently expect field)
- [x] vitest: palette dispatch carries the captured pre-open scope chain (red: dispatches with palette/live chain); picking inspect dispatches with the entity chain
- [x] Prefix mirror tests updated if INSPECTABLE_ENTITY_PREFIXES changes (both ends of the pinned chain)
- [x] cargo nextest -p swissarmyhammer-command-service green; scoped vitest + tsc green (one PRE-EXISTING unrelated failure `meta_tree_id_param_is_required_where_expected` fails on committed HEAD — filed as card 01KTY97TWP9BJB4CX53H8CYBF5)

## Constraints
- NO whole-workspace cargo build/clippy; crate-scoped only. tauri dev hot-reloads.
- One data-driven rule — no per-command or per-entity special cases in React (metadata-driven-ui, commands-in-rust).
- Do NOT touch .kanban/actors/wballard.jsonl.

## Workflow
- Use `/tdd` — failing tests first at the seams the forensics implicate.