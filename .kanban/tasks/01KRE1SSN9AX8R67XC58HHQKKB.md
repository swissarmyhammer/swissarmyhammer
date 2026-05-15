---
assignees:
- claude-code
depends_on:
- 01KRE1R8AWHXM385ZFJJKXW2XB
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffd680
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

- [x] `OptionsResolver` trait and `OptionsRegistry` exist with the documented surface.
- [x] `PerspectiveFieldsResolver`, `ViewKindsResolver`, and `SortDirectionsResolver` are registered as built-ins.
- [x] `commands_for_scope` populates `options` on every enum-shaped param whose `options_from` is registered.
- [x] A param with `options_from: "perspective.fields"` and no `perspective:{id}` in scope resolves to an empty list, NOT a panic.
- [x] A param with `options_from: "nonexistent.resolver"` leaves `options: None` and logs a `warn!` once per command emission.
- [x] `cargo test -p swissarmyhammer-kanban` passes.

## Tests

- [x] Unit test in `swissarmyhammer-kanban/src/commands/options_resolvers.rs`: `perspective_fields_resolver_returns_fields_for_in_scope_perspective` — build a `DynamicSources` with one perspective carrying three fields, scope chain `["perspective:01P"]`, assert the resolver returns three `ParamOption`s in field order with `value = field_id` and `label = field_display_name`.
- [x] Unit test: `perspective_fields_resolver_returns_empty_when_no_perspective_in_scope` — same fixture, scope chain `[]`, assert `Vec::new()`.
- [x] Unit test: `view_kinds_resolver_lists_every_variant_via_canonical_helper` — iterate every `ViewKind` variant, assert each appears in the resolver output with the kebab-case value from `ViewKind::as_kebab_str()`. (Pairs with the existing exhaustiveness test in `swissarmyhammer-views`.)
- [x] Unit test: `sort_directions_resolver_returns_asc_and_desc_only` — exact-match the two-entry list.
- [x] Integration test in `swissarmyhammer-kanban/tests/options_enrichment.rs` (new): `commands_for_scope_populates_enum_options` — register a synthetic command `test.pick.field` with `shape: enum, options_from: "perspective.fields"`, emit it through `commands_for_scope` in a perspective-bearing scope, assert the emitted command carries the populated `options` field non-empty.
- [x] Integration test: `commands_for_scope_leaves_options_none_for_unknown_resolver` — synthetic command with `options_from: "nonexistent.resolver"`, assert emitted command's `options` is `None`.
- [x] Run: `cargo test -p swissarmyhammer-kanban` — green.

## Workflow

- Use `/tdd` — write the unit tests for the three built-in resolvers first, then the integration tests for the `commands_for_scope` enrichment step, then implement.
- Use `code_context get blastradius` on `commands_for_scope` before editing — the previous epic added a `filter_by_view_kind` pass; ensure the options-enrichment pass runs AFTER all filtering so we don't waste work resolving options for commands that get dropped.
- Resolvers should NOT hold their own state — they read from `OptionsContext`. This keeps them trivially clonable / sendable for test fixtures.

## Implementation notes

- The `OptionsResolver` trait + `OptionsRegistry` + `OptionsContext` live in `swissarmyhammer-commands/src/options_resolver.rs` as planned. To respect the existing crate dependency graph (`swissarmyhammer-commands` does NOT depend on `swissarmyhammer-kanban`), `OptionsContext::data` is a `&dyn std::any::Any` that kanban-side resolvers downcast to `&DynamicSources`. This keeps the trait/registry consumer-agnostic while still letting kanban resolvers reach the kanban-specific runtime data.
- `OptionsRegistry::new()` in the commands crate constructs an EMPTY registry. The kanban built-in resolvers are registered by `swissarmyhammer_kanban::commands::options_resolvers::default_options_registry()`, which the `KanbanContext` constructor wires into `options_registry()`. This preserves the "kanban contributes its own resolvers" semantics without forcing the consumer-agnostic crate to know about perspectives or views.
- `commands_for_scope` gained an 8th parameter `options_registry: Option<&OptionsRegistry>`. The enrichment pass runs AFTER `filter_by_view_kind` (as specified). Callers that don't need resolution (e.g. `kanban-app::menu::resolve_command_availability` for the native menu bar) pass `None`; `kanban-app::commands::list_commands_for_scope` threads `KanbanContext::options_registry()` through.
- `ResolvedCommand` now carries `params: Vec<ParamDef>` (the post-enrichment param list). Synthetic / dynamic rows (`view.set` fan-out, `board.switch:{path}`, `window.focus:{label}`, `perspective.set` fan-out, `entity.add:{type}`) carry an empty params list — they have no user-pickable params. Cross-cutting, scoped-registry, and global-registry emits all copy `CommandDef.params` verbatim; the enrichment pass mutates `params[].options` in place.
- `PerspectiveInfo` (in `scope_commands.rs`) gained a `fields: Vec<PerspectiveFieldInfo>` denormalised at `gather_perspectives` time by joining `Perspective.fields[].field` (field ULID) against the active board's `FieldsContext`. This lets `PerspectiveFieldsResolver` answer at resolve-time without needing `FieldsContext` itself.
- **Warn-once-per-PROCESS for unknown `options_from` keys** (was per-call) is implemented via a module-level `static LOGGED_MISSING_OPTIONS_RESOLVERS: Mutex<Option<HashSet<String>>>` keyed on a `"{cmd_id}|{param_name}|{options_from_key}"` triple, mirroring the canonical `LOGGED_LEGACY_PERSPECTIVES` shape in `swissarmyhammer-kanban/src/perspective/migrate.rs`. A `#[cfg(any(test, feature = "test-support"))] reset_missing_options_resolvers_log_guard_for_test()` companion mirrors `reset_legacy_log_guard_for_test`. Updated 2026-05-12 in response to review.

## Review Findings (2026-05-12 13:30)

### Warnings

- [x] `swissarmyhammer-kanban/src/scope_commands.rs:737` — `enrich_options`'s warn dedupe is **per-call**, not **per-process**. The `HashSet<(String, String, String)>` is allocated fresh inside the function on every `commands_for_scope` invocation, so every focus change / right-click / palette open will re-emit the same "no resolver registered" warning. The established pattern in this codebase for one-shot diagnostic logs is a process-wide `static Mutex<Option<HashSet<String>>>` — see `swissarmyhammer-kanban/src/perspective/migrate.rs:90` (`LOGGED_LEGACY_PERSPECTIVES`) for the canonical shape. The implementer's own task body says "the right scope is 'per unique key, per process'" — this should be promoted to a module-level static keyed on `(cmd_id, param_name, options_from_key)` so the warning fires exactly once per missing key for the life of the process.
  - **Resolved 2026-05-12**: replaced the per-call `HashSet` with a module-level `static LOGGED_MISSING_OPTIONS_RESOLVERS: Mutex<Option<HashSet<String>>>` keyed on the `"{cmd_id}|{param_name}|{options_from_key}"` triple. The warn logic is now factored into a `warn_once_unknown_resolver` helper. A test-only `reset_missing_options_resolvers_log_guard_for_test()` mirrors the existing `reset_legacy_log_guard_for_test` so future tests that capture logs can re-enter the helper with a fresh guard.

- [x] `swissarmyhammer-kanban/tests/options_enrichment.rs` — None of the four integration tests, nor the unit tests in `scope_commands.rs`, exercise both `view_kinds` filtering AND `options_from` enrichment simultaneously. If `enrich_options` and `filter_by_view_kind` were swapped at lines 684/690, every existing test would still pass. The task spec is explicit that enrichment must run AFTER filtering so resolvers don't pay for commands that get dropped — add a test that registers a command with `view_kinds: [grid]` + an `options_from`-tagged param, emits it under a `view:{id}` whose kind is `board`, and asserts the command is dropped AND that no resolver was invoked for that command. The "no resolver was invoked" half requires a counting test resolver; the bare "command is dropped" assertion is enough to lock the ordering contract at minimum.
  - **Resolved 2026-05-12**: added `commands_for_scope_skips_options_resolution_for_view_kind_filtered_commands` in `swissarmyhammer-kanban/tests/options_enrichment.rs`. The test installs a `CountingResolver` (a fixture `OptionsResolver` that increments an `AtomicUsize` per `resolve` call), registers a synthetic command tagged with both `view_kinds: [grid]` and `options_from: "test.counting"`, emits under a board-kind view, and asserts (a) the command is filtered out of the result AND (b) the counter is 0 — locking in BOTH halves of the ordering claim.

### Nits

- [ ] `swissarmyhammer-kanban/src/scope_commands.rs:641-650` — `commands_for_scope` is now an 8-parameter function with `#[allow(clippy::too_many_arguments)]`. Call sites like `commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None, None)` appear 80+ times in tests and are hard to read at a glance — three of the eight values are unit/sentinel values whose meaning is purely positional. Not a blocker for this task (the change merely adds one more None), but consider bundling the inputs into a `CommandsForScopeArgs { registry, command_impls, fields, ui_state, context_menu_only, dynamic, options_registry }` struct in a follow-up. The scope chain is the only argument that varies per call in most call-sites.
  - **Deferred (reviewer-marked follow-up)**: the reviewer explicitly tagged this as "Not a blocker for this task" and a candidate for a separate follow-up refactor. The 80+ call-site sweep is large enough to warrant its own task so the diff stays reviewable. Leaving as-is for now.

- [ ] `swissarmyhammer-kanban/src/scope_commands.rs:117-148` — `PerspectiveInfo.fields` denormalises every perspective's field list at `gather_perspectives` time even when no command consumes `perspective.fields`. The cost is small (perspectives are bounded; field count is bounded) but it couples two unrelated concerns: `PerspectiveInfo` was originally about producing `perspective.set` palette rows, and now carries picker-resolver source data. An alternative that keeps `PerspectiveInfo` minimal is to thread the `FieldsContext` into the `OptionsContext` (alongside `DynamicSources`) and have `PerspectiveFieldsResolver` look up the field metadata lazily at resolve time — only paying the cost when a picker actually asks. Accept as-is, but note for the next epic refactor.
  - **Deferred (reviewer-marked follow-up)**: the reviewer explicitly said "Accept as-is, but note for the next epic refactor". Changing `OptionsContext` to carry `FieldsContext` would ripple through every resolver signature in `swissarmyhammer-commands`, which is out of scope for this task and belongs to the next epic refactor as the reviewer suggested. Leaving as-is for now.

- [x] `swissarmyhammer-kanban/src/commands/options_resolvers.rs:55-62` — The doc comment on `PerspectiveFieldsResolver` contains a self-debating "Walks `scope_chain` outermost-to-innermost? Actually …" — that reads like an unfinished thought left in. Rewrite as a declarative statement: "Walks the scope chain innermost-first (the documented order from `commands_for_scope`) and returns the first `perspective:{id}` it encounters." Same paragraph, no scratchpad voice.
  - **Resolved 2026-05-12**: rewrote the paragraph as the reviewer requested — single declarative statement, no scratchpad voice.