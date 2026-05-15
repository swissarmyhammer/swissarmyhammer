---
assignees:
- claude-code
position_column: todo
position_ordinal: 9c80
title: 'Resolve field/group from field: scope moniker in sort and group commands'
---
## What

Right-clicking a tag cell (or any grid cell) and picking "Sort Field" fails with `perspective.sort.set failed: missing required arg: field`. The scope chain carries a `field:{entity_type}:{entity_id}.{field_name}` moniker (built by `fieldMoniker` in `kanban-app/ui/src/lib/moniker.ts` and pushed by grid cell scope providers), but `SetSortCmd`, `ToggleSortCmd`, and `SetGroupCmd` in `swissarmyhammer-kanban/src/commands/perspective_commands.rs` only read `ctx.arg("field")` / `ctx.arg("group")` — they never consult the scope chain.

The command-context helpers already expose scope-chain resolution for entity types (`CommandContext::resolve_entity_id`, `resolve_moniker`) and the perspective mutation commands already resolve `perspective_id` via a well-established arg → scope → UIState → fallback chain in `resolve_perspective_id`. The field-name argument has no equivalent chain: today it is args-only, so any menu click without an explicit `field` arg dies at the `ok_or_else(|| MissingArg("field".into()))` line.

The fix is to add a field-name scope resolver and use it as the fallback in the three commands that need a schema field name. Same thing the perspective-id resolver does, scoped to field-monikers instead.

### Files to modify

1. `swissarmyhammer-commands/src/context.rs` — add a helper:
   ```rust
   /// Resolve a schema field name from a `field:{type}:{id}.{name}` moniker in
   /// the scope chain.
   ///
   /// Returns the portion after the last `.` of the innermost `field:` moniker
   /// (the schema field name, not the full moniker). Returns None when no
   /// `field:` moniker is present or the moniker has no `.field` suffix.
   pub fn resolve_field_name(&self) -> Option<&str> {
       let (_t, rest) = self.resolve_moniker("field")?;
       let dot = rest.rfind('.')?;
       let name = &rest[dot + 1..];
       if name.is_empty() { None } else { Some(name) }
   }
   ```
   The helper lives next to `resolve_entity_id`, uses only existing primitives (`resolve_moniker`), and mirrors the parse semantics of `parseFieldMoniker` on the frontend. Already-tested invariants (`parse_moniker("field:task:abc.title")` → `("field", "task:abc.title")`) stay unchanged.

2. `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — add a local resolver helper that captures the arg → scope pattern once, then use it in three places:

   ```rust
   /// Resolve a schema field name argument the same way `resolve_perspective_id`
   /// resolves the perspective id: explicit arg first, then the innermost
   /// `field:` moniker in the scope chain.
   fn resolve_field_arg<'a>(
       ctx: &'a CommandContext,
       arg_name: &str,
   ) -> Result<&'a str, CommandError> {
       if let Some(v) = ctx.arg(arg_name).and_then(|v| v.as_str()) {
           return Ok(v);
       }
       ctx.resolve_field_name()
           .ok_or_else(|| CommandError::MissingArg(arg_name.into()))
   }
   ```

   - `SetGroupCmd::execute` — replace the args-only read of `group` with `resolve_field_arg(ctx, "group")`.
   - `ToggleSortCmd::execute` — replace the args-only read of `field` with `resolve_field_arg(ctx, "field")`.
   - `SetSortCmd::execute` — replace the args-only read of `field` with `resolve_field_arg(ctx, "field")`.

   Direction is a second problem for `SetSortCmd` only. A context-menu click has no direction input, so `SetSortCmd` cannot sensibly be a context-menu entry. Fix by **routing the context-menu surface to `perspective.sort.toggle`** (which cycles asc → desc → none and needs only `field`) and keeping `perspective.sort.set` as a palette-only command where `direction` must be explicit. Concretely:
   - In `swissarmyhammer-commands/builtin/commands/perspective.yaml`, remove `context_menu: true` from `perspective.sort.set`. Leave it on `perspective.sort.toggle`.
   - Update the display name of `perspective.sort.toggle` in the YAML from `Toggle Sort` to `Sort Field` so the context-menu label matches what the user expects to see on a field cell. The palette keeps `perspective.sort.set` as `Sort Field` for the "pick direction explicitly" surface.
   - (Alternative we are rejecting: defaulting `direction` to `"asc"` inside `SetSortCmd`. That would silently overload a command whose entire point is explicit direction, and two commands that do subtly different things under the same name is the opposite of minimalism.)

3. `swissarmyhammer-commands/builtin/commands/perspective.yaml` — reflect the new resolution source in the metadata so the schema is self-documenting:
   - On `perspective.group`: change the `group` param's `from: args` to `from: scope_chain` and add `entity_type: field`.
   - On `perspective.sort.set`: change the `field` param the same way.
   - On `perspective.sort.toggle`: change the `field` param the same way; update `name:` from `Toggle Sort` to `Sort Field`; remove `context_menu: true` from `perspective.sort.set` (see above).

   This is metadata-only at present (the current runtime does not auto-populate args from `from: scope_chain` declarations — each command resolves in code), but aligning the YAML keeps the declaration honest and positions the param layer for a future declarative resolver.

### Out of scope

- `perspective.filter` / `perspective.clearFilter` — the `filter` arg is a DSL expression, not a field name, and has no useful scope-chain source. Leave untouched.
- `perspective.clearSort` / `perspective.clearGroup` — neither needs a field name. Leave untouched.
- Auto-populating command args from `ParamDef.from: scope_chain` at dispatch time. That is a bigger architectural change; the YAML updates here are documentation for that future work, and the runtime fix stays in command-body code matching the existing `resolve_perspective_id` pattern.
- Deriving `direction` for `SetSortCmd` from any source other than explicit `args.direction`. See the design rationale in the Files section.

### Why this shape

`resolve_perspective_id` already establishes the arg → scope → UIState → fallback pattern for the perspective id. The `field` and `group` arguments have exactly the same structure — there's always an explicit args path (palette, keybinding with args, header click that already passes `field` explicitly) and there's often a scope-chain path (right-click on a cell or header). Mirroring the existing helper is the principled fix; adding a per-command special case or defaulting direction would not.

## Acceptance Criteria

- [ ] Right-clicking a grid cell rendered over a tag or other field column and picking "Sort Field" from the context menu toggles that field's sort on the active perspective. No `missing required arg` error.
- [ ] Right-clicking the same cell and picking "Group" / "Group By" sets the perspective's group to that field. No `missing required arg` error.
- [ ] Invoking `perspective.sort.set` from the palette without `direction` still errors with `MissingArg("direction")` — the command keeps its explicit-direction semantics, just loses the `context_menu: true` flag.
- [ ] `perspective.sort.toggle` with `field` explicit (palette, direct API) continues to behave exactly as today.
- [ ] Direct-click on a grid column header (the existing `dispatchSortToggle` path in `kanban-app/ui/src/components/data-table.tsx` that passes `{ field, perspective_id }` explicitly) continues to work — the scope-chain fallback is additive, not a replacement.
- [ ] The context menu on a field-scoped target shows "Sort Field" (the renamed `perspective.sort.toggle`) and no longer shows a separate entry from the now-non-context-menu `perspective.sort.set`.

## Tests

- [ ] `swissarmyhammer-commands/src/context.rs` — new test `resolve_field_name_extracts_suffix_after_last_dot`: context with scope `["field:task:abc.tag_name", "task:abc", "column:todo"]`, assert `ctx.resolve_field_name() == Some("tag_name")`.
- [ ] `swissarmyhammer-commands/src/context.rs` — new test `resolve_field_name_returns_none_without_field_moniker`: scope `["task:abc", "column:todo"]`, assert `None`.
- [ ] `swissarmyhammer-commands/src/context.rs` — new test `resolve_field_name_innermost_wins`: scope `["field:task:abc.title", "field:task:abc.priority"]`, assert `Some("title")` (innermost first matches `resolve_moniker`'s existing contract).
- [ ] `swissarmyhammer-commands/src/context.rs` — new test `resolve_field_name_returns_none_on_dotless_rest`: scope `["field:task:abc"]` (malformed — no dot), assert `None`.
- [ ] `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — new test `toggle_sort_resolves_field_from_scope_when_arg_missing`: build a context with no `field` arg and scope chain including `field:task:abc.priority`, dispatch `ToggleSortCmd`, assert the persisted perspective's sort list now has a `priority` entry.
- [ ] `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — new test `set_sort_resolves_field_from_scope_when_arg_missing`: supply `direction: "asc"` but no `field`; scope chain has `field:task:abc.priority`; assert the perspective sort has `priority asc`.
- [ ] `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — new test `set_group_resolves_group_from_scope_when_arg_missing`: no `group` arg, scope chain has `field:task:abc.status`, assert the perspective's group is now `"status"`.
- [ ] `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — new test `set_sort_still_requires_direction_from_palette`: explicit `field` arg, no `direction`, empty scope — assert `MissingArg("direction")`. Guards against an accidental direction default being added later.
- [ ] `cargo test -p swissarmyhammer-commands` and `cargo test -p swissarmyhammer-kanban` both pass.
- [ ] Snapshot fixtures under `swissarmyhammer-kanban/tests/snapshots/` that include the removed `context_menu: true` on `perspective.sort.set` and the renamed `perspective.sort.toggle` are regenerated. Verify `board_full.json`, `board_context_menu_only.json`, and the grid-scoped variants diff as expected (no `perspective.sort.set` under context-menu-only; `perspective.sort.toggle` shows as name "Sort Field").
- [ ] Manual verification in the running app: on a grid view, right-click a cell under a user-defined field column; confirm "Sort Field" appears once in the menu, clicking toggles the sort, second click flips direction, third click clears — the cycle provided by `ToggleSortCmd`. Confirm "Group" / "Group By" on the same menu sets the group to that field.

## Workflow

- Use `/tdd` — start with the four `context.rs` helper tests, make them red, implement `resolve_field_name`, green. Then the four command tests red → green with `resolve_field_arg` and the three call-site updates.
- Update the YAML last, after runtime tests pass, so any snapshot churn shows up together and can be inspected in one diff.
- Finish with a manual run: start kanban-app, open a grid perspective, right-click a cell, verify "Sort Field" and "Group" work without arguments.