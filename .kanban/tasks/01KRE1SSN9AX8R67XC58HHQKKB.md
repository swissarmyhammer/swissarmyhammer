---
assignees:
- claude-code
depends_on:
- 01KRE1R8AWHXM385ZFJJKXW2XB
position_column: todo
position_ordinal: a380
title: Backend resolver registry for param options_from
---
## What

Now that `ParamDef.options_from: Option<String>` exists (prerequisite task), wire up the backend resolver that turns those stringly-typed keys into concrete `options: Vec<ParamOption>` at `commands_for_scope` emission time. The frontend never invents picker options — it consumes whatever the backend embedded.

### Files to modify

- `swissarmyhammer-commands/src/options_resolver.rs` (new) — trait + registry:
  ```rust
  /// Resolves an `options_from` key into a concrete list of options
  /// for a specific scope. Implementations read from existing
  /// kanban context (fields, views, projects, …) and return the
  /// labelled value list the frontend will render in a picker.
  pub trait OptionsResolver: Send + Sync {
      fn key(&self) -> &'static str;
      fn resolve(&self, ctx: &OptionsContext) -> Vec<ParamOption>;
  }

  pub struct OptionsRegistry { /* … */ }
  impl OptionsRegistry {
      pub fn new() -> Self { /* registers built-ins */ }
      pub fn register(&mut self, r: Box<dyn OptionsResolver>);
      pub fn resolve(&self, key: &str, ctx: &OptionsContext) -> Option<Vec<ParamOption>>;
  }
  ```
  `OptionsContext` carries `&DynamicSources` plus the scope chain, so resolvers can ask "what fields does this perspective have?" or "what view kinds exist?" without owning the data themselves.

- `swissarmyhammer-kanban/src/commands/options_resolvers.rs` (new) — built-in resolvers:
  - `PerspectiveFieldsResolver` (key = `"perspective.fields"`) — resolves the innermost `perspective:{id}` moniker from the scope chain, looks the perspective up in `DynamicSources.perspectives`, returns its sortable/groupable field list as `ParamOption { value: field_id, label: field_display_name }`.
  - `ViewKindsResolver` (key = `"view.kinds"`) — static list `[Board, Grid, List, Calendar, Timeline]` projected through `ViewKind::as_kebab_str()` so the labels stay coherent with the rest of the codebase.
  - `SortDirectionsResolver` (key = `"sort.directions"`) — static `[("asc", "Ascending"), ("desc", "Descending")]`.

  Built-ins are registered by `OptionsRegistry::new()`; downstream crates can `register()` additional resolvers.

- `swissarmyhammer-kanban/src/scope_commands.rs` — `commands_for_scope` (or whichever helper emits the final `CommandDef` list) walks each emitted command's `params[]`. For every param with `shape: Some(Enum)` and `options_from: Some(key)`, look up the resolver in the registry, call `resolve(&ctx)`, and write the result into the param's `options` field on the emitted command. Already-inline `options` are left alone.

  Construct the `OptionsContext` once per `commands_for_scope` invocation; share it across all param resolutions in that call.

- `swissarmyhammer-kanban/src/context.rs` (or wherever `KanbanContext` is wired) — hold the `OptionsRegistry` so `commands_for_scope` can reach it. Default-construct with the built-ins on context init.

### Behavior

- Commands emitted by `commands_for_scope` carry concrete `options` for every enum-shaped, `options_from`-tagged param.
- A `options_from` key with no registered resolver leaves the param's `options` as `None`. The frontend MUST treat that as "this command can't be picked right now" — render the button but disable it, or skip it entirely (separate UI task to decide).
- Resolution is eager: every command in every emission carries its options. The cost is bounded — perspective fields are already in memory; view kinds and sort directions are constants.

### Out of scope

- Frontend rendering of options — separate `<CommandPopover>` task.
- Annotating individual commands with `options_from` — handled per migration.

## Acceptance Criteria

- [ ] `OptionsResolver` trait and `OptionsRegistry` exist with the documented surface.
- [ ] `PerspectiveFieldsResolver`, `ViewKindsResolver`, and `SortDirectionsResolver` are registered as built-ins.
- [ ] `commands_for_scope` populates `options` on every enum-shaped param whose `options_from` is registered.
- [ ] A param with `options_from: "perspective.fields"` and no `perspective:{id}` in scope resolves to an empty list, NOT a panic.
- [ ] A param with `options_from: "nonexistent.resolver"` leaves `options: None` and logs a `warn!` once per command emission.
- [ ] `cargo test -p swissarmyhammer-kanban` passes.

## Tests

- [ ] Unit test in `swissarmyhammer-kanban/src/commands/options_resolvers.rs`: `perspective_fields_resolver_returns_fields_for_in_scope_perspective` — build a `DynamicSources` with one perspective carrying three fields, scope chain `["perspective:01P"]`, assert the resolver returns three `ParamOption`s in field order with `value = field_id` and `label = field_display_name`.
- [ ] Unit test: `perspective_fields_resolver_returns_empty_when_no_perspective_in_scope` — same fixture, scope chain `[]`, assert `Vec::new()`.
- [ ] Unit test: `view_kinds_resolver_lists_every_variant_via_canonical_helper` — iterate every `ViewKind` variant, assert each appears in the resolver output with the kebab-case value from `ViewKind::as_kebab_str()`. (Pairs with the existing exhaustiveness test in `swissarmyhammer-views`.)
- [ ] Unit test: `sort_directions_resolver_returns_asc_and_desc_only` — exact-match the two-entry list.
- [ ] Integration test in `swissarmyhammer-kanban/tests/options_enrichment.rs` (new): `commands_for_scope_populates_enum_options` — register a synthetic command `test.pick.field` with `shape: enum, options_from: "perspective.fields"`, emit it through `commands_for_scope` in a perspective-bearing scope, assert the emitted command carries the populated `options` field non-empty.
- [ ] Integration test: `commands_for_scope_leaves_options_none_for_unknown_resolver` — synthetic command with `options_from: "nonexistent.resolver"`, assert emitted command's `options` is `None`.
- [ ] Run: `cargo test -p swissarmyhammer-kanban` — green.

## Workflow

- Use `/tdd` — write the unit tests for the three built-in resolvers first, then the integration tests for the `commands_for_scope` enrichment step, then implement.
- Use `code_context get blastradius` on `commands_for_scope` before editing — the previous epic added a `filter_by_view_kind` pass; ensure the options-enrichment pass runs AFTER all filtering so we don't waste work resolving options for commands that get dropped.
- Resolvers should NOT hold their own state — they read from `OptionsContext`. This keeps them trivially clonable / sendable for test fixtures. #command-driven-ui