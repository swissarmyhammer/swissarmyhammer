---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff9380
title: Audit ui.perspective.startRename for kanban-domain leak
---
## What

Follow-up from **01KPY02X405QTP5ACH67THHSN8** ("Move ui.view.set / ui.perspective.set out of generic ui.yaml into kanban domain"). That task deliberately limited scope to the `.set` commands. During review, the reviewer noted that the id `ui.perspective.startRename` still survives in `swissarmyhammer-commands/builtin/commands/ui.yaml` despite the acceptance-criterion grep wording ("grepping [`ui.yaml`] for `view`, `perspective`, `board`, `task`, `column`, `tag`, `attachment` turns up nothing").

The `.set` commands mutate domain state and were straightforward to relocate. `ui.perspective.startRename` is functionally different: it is a UI primitive that the frontend intercepts before the backend no-op implementation fires. It toggles inline-rename mode on a focused perspective tab. Whether it belongs under the `ui.` namespace or the `perspective.` namespace is a judgment call that deserves its own conversation.

### Decision: Option 1 — rename to `ui.entity.startRename`

Inline-rename is a generic UI mechanic that conceptually applies to any focusable entity — perspectives today, tasks/tags/actors tomorrow. The frontend already intercepts this command before the backend no-op impl fires (the backend command exists only so it appears in the palette and picks up a keybinding). Renaming to `ui.entity.startRename` keeps it in the generic UI crate where the palette/keybinding plumbing lives, while shedding the kanban-specific `.perspective.` segment from the id. If future entity types need distinct semantics, the frontend interceptor can branch on the focused entity's type.

The new id establishes a **new** `ui.entity.*` convention rather than extending the backend `entity.*` one. The backend `entity.*` namespace (`entity.add`, `entity.delete`, `entity.archive`, `entity.cut`, `entity.paste`) has specific contract semantics: primary param `moniker` `from: target`, auto-emitted per scope-chain moniker, per-type Rust `Command::available()` opt-out, backend-dispatched. `ui.entity.startRename` follows none of those — it has no target, no entity_type arg, no scope-chain auto-emit, and is frontend-intercepted. The `ui.entity.` prefix telegraphs *UI-layer and focus-bound* without claiming alignment with the backend `entity.*` contract. It is currently the sole `ui.entity.*` command; future focus-bound UI primitives could join it under the same prefix.

### Current usage sites (from grep)

```
swissarmyhammer-commands/builtin/commands/ui.yaml        (1 match) — YAML declaration
swissarmyhammer-commands/src/registry.rs                 (1 match) — `ui_yaml_arg_only_commands_are_hidden_from_palette` hidden-id list
swissarmyhammer-kanban/src/commands/ui_commands.rs       (2 matches) — register + test
swissarmyhammer-kanban/src/commands/mod.rs               (1 match) — registry insert
swissarmyhammer-kanban/builtin/commands/perspective.yaml (1 match) — cross-reference/comment
swissarmyhammer-kanban/tests/builtin_commands.rs         (1 match) — test assertion
kanban-app/ui/src/components/app-shell.tsx               (2 matches) — dispatch sites
kanban-app/ui/src/components/perspective-tab-bar.tsx     (1 match) — dispatch site
kanban-app/ui/src/components/perspective-tab-bar.test.tsx (1 match) — test expectation
```

### Out of scope

- Any other `ui.*` ids that reference kanban concepts; if found, file a separate follow-up.
- Moving the backend impl out of `swissarmyhammer-kanban/src/commands/ui_commands.rs` (rename the id and register_* function, but the impl stays in the same module since no kanban-side `perspective_commands.rs` currently exists for no-op handlers).

## Acceptance Criteria

- [x] A decision is documented in this task's description (option 1, 2, or 3 above), with rationale.
- [x] Either (a) the id is moved (option 2), (b) it's renamed to a domain-neutral id (option 1), or (c) the task is closed with a rationale for keeping it as-is (option 3).
- [x] If a rename is chosen, all 9 files listed above are updated and `rg 'ui\.perspective\.startRename'` returns zero hits outside git history.
- [x] `cargo test -p swissarmyhammer-commands -p swissarmyhammer-kanban -p kanban-app` passes.
- [x] `pnpm -C kanban-app/ui test --run` passes.

#commands #organization

## Review Findings (2026-04-23)

The rename is correct and tests pass. Two minor cleanups to fold in before marking done:

- [x] **Stale enumeration comments.** Two doc comments still name the command by shorthand `perspective.startRename` even though the segment no longer exists. The acceptance grep (`ui\.perspective\.startRename`) doesn't catch them because they use the shorthand. Update:
  - `swissarmyhammer-commands/src/registry.rs` — inside the `builtin_yaml_files_parse` test's per-file breakdown comment, the `ui:` line reads `… palette.close, perspective.startRename, setFocus, window.new, mode.set = 9`. Change to `entity.startRename`. **Resolved**: comment updated to `entity.startRename`.
  - `swissarmyhammer-kanban/src/commands/mod.rs` — inside `register_commands_returns_expected_count`'s doc comment, the `8 UI (…)` line reads `… palette.close, perspective.startRename, setFocus, mode.set`. Change to `entity.startRename`. **Resolved**: comment updated to `entity.startRename`.
- [x] **Optional: soften "namespace fit" rationale.** The description claims `ui.entity.startRename` "fits the existing `entity.*` namespace convention." On closer reading of `swissarmyhammer-commands/builtin/commands/entity.yaml`, the `entity.*` convention is specifically: primary param `moniker` `from: target`, auto-emitted per scope-chain moniker, per-type Rust `Command::available()` opt-out, backend-dispatched. `ui.entity.startRename` follows none of these — it has no target, no entity_type arg, no scope chain auto-emit, and is frontend-intercepted. It's the sole `ui.entity.*` command and therefore creates a **new** convention, not an extension of the backend `entity.*` one. The forward-looking "future entity types can branch on the focused entity's type in the frontend interceptor" is aspirational: the current interceptor is an unconditional `triggerStartRename()` with no focus-context read. Either reword to "the `ui.entity.` prefix telegraphs UI-layer and focus-bound without claiming alignment with backend `entity.*` contract semantics" or accept this review note as historical context — either is fine. Not blocking. **Resolved**: rationale paragraph in the Decision section reworded to establish `ui.entity.*` as a new UI-layer convention, explicitly calling out the backend `entity.*` contract semantics it does not share.

Outcome: once the two comments in Finding 1 are cleaned up, this is ready to close.