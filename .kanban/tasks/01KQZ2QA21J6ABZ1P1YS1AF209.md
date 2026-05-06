---
assignees:
- claude-code
position_column: todo
position_ordinal: e480
project: spatial-nav
title: Compose builtin commands at the app layer via a macro
---
## What

Replace the hand-stitched two-crate aggregator in `swissarmyhammer-kanban/src/lib.rs:64-71` (`default_builtin_yaml_sources` reaches into the commands crate) with a macro at the app layer. Each contributor crate ships its YAML directory plus a `pub fn builtin_yaml_sources()`, and the **app** crate (kanban-app, kanban-cli, mirdan-app) decides which contributors to compose and in what order via the macro.

### Dep direction

The dep direction is `contributor → swissarmyhammer-commands` (registry crate). Same as today's kanban → commands. The focus crate, when it adds its own YAML in the next task, will also depend on commands. That's expected and natural — there's no cycle because commands does not import any contributor crate. What this task fixes is the *aggregation* sitting in the wrong place: today `swissarmyhammer-kanban` is the aggregator (a domain crate composing across other crates' commands); after this task the app is the aggregator and the kanban crate is just another contributor.

### Concrete shape

1. **In `swissarmyhammer-commands/src/lib.rs`**, add a `#[macro_export]` macro:

   ```rust
   /// Compose a `CommandsRegistry` from the `builtin_yaml_sources()`
   /// functions exposed by each listed crate, in order.
   ///
   /// Each crate must export `pub fn builtin_yaml_sources() ->
   /// Vec<(&'static str, &'static str)>`. The order in the macro IS
   /// the partial-merge precedence: later sources override earlier
   /// by id.
   ///
   /// # Example
   /// ```ignore
   /// let registry = compose_registry![
   ///     swissarmyhammer_commands,  // generic UI commands
   ///     swissarmyhammer_focus,     // navigation
   ///     swissarmyhammer_kanban,    // domain (overrides allowed)
   /// ];
   /// ```
   #[macro_export]
   macro_rules! compose_registry {
       ($($crate_path:path),+ $(,)?) => {{
           let mut sources: Vec<(&'static str, &'static str)> = Vec::new();
           $( sources.extend($crate_path::builtin_yaml_sources()); )+
           $crate::CommandsRegistry::from_yaml_sources(&sources)
       }};
   }
   ```

   Also add a sibling macro `compose_yaml_sources!` that returns the flat `Vec<(&'static str, &'static str)>` instead of a registry, for callers that need to layer on user overrides:

   ```rust
   #[macro_export]
   macro_rules! compose_yaml_sources {
       ($($crate_path:path),+ $(,)?) => {{
           let mut sources: Vec<(&'static str, &'static str)> = Vec::new();
           $( sources.extend($crate_path::builtin_yaml_sources()); )+
           sources
       }};
   }
   ```

2. **In `kanban-app/src/state.rs`** — wherever `swissarmyhammer_kanban::default_commands_registry()` is currently called (`state.rs:476` etc.), replace it with:

   ```rust
   let registry = swissarmyhammer_commands::compose_registry![
       swissarmyhammer_commands,
       swissarmyhammer_kanban,
   ];
   ```

   (`swissarmyhammer_focus` joins this list in the next task once the focus crate ships its `nav.yaml`.)

   For the with-overrides variant, use `compose_yaml_sources!` and append loaded user overrides before constructing the registry (mirror what `default_commands_registry_with_overrides` does today).

3. **In `kanban-cli`** — same change. Confirm by reading the CLI's current registry construction; whichever subsystems it pulls in stay in. (Likely just commands + kanban; no focus.)

4. **In `swissarmyhammer-kanban/src/lib.rs`** — remove `default_builtin_yaml_sources`, `default_commands_registry`, and `default_commands_registry_with_overrides`. They're no longer needed; the app does the composition. Keep `builtin_yaml_sources()` (the kanban crate's contribution function — same shape every contributor exposes).

5. **`swissarmyhammer-commands/src/lib.rs`** — keep `builtin_yaml_sources()` exposed. The commands crate is now just one contributor among many, contributed via the macro at the app layer like everyone else.

6. **No changes to `swissarmyhammer-focus`** in this task. The focus crate adds its `builtin/commands/nav.yaml` and `builtin_yaml_sources()` in the next task; until then `compose_registry!` is invoked with just commands + kanban.

7. **Stragglers**: `grep -rn "default_commands_registry\b" --include="*.rs"` and update every caller. The existing test at `swissarmyhammer-kanban/tests/default_commands_registry.rs` needs to be updated; the underlying assertion (registry contents) is still useful — refactor it to use the macro and rename if needed.

### Why a macro and not a function-pointer table

A const slice of function pointers (`const SOURCES: &[fn() -> Vec<...>] = &[...]`) would also work and avoid macros. The macro is preferred because:
- It returns a `CommandsRegistry` directly (not an intermediate `Vec`), so the call site is one line instead of three.
- The contributor list is right at the call site, not in some other file.
- The macro handles the trailing-comma case and produces a readable error if a contributor doesn't expose `builtin_yaml_sources()`.

### Why not `linkme` / `inventory`

Considered and dropped. Both push the registration into the contributor crate via macros that need to coordinate with the registry crate. The plain-macro approach keeps contributor crates as simple data-providers and puts the composition decision at the app layer where it belongs — the app knows which subsystems it includes.

## Acceptance Criteria

- [ ] `swissarmyhammer-commands` exports `compose_registry!` and `compose_yaml_sources!` macros.
- [ ] `kanban-app/src/state.rs` constructs its registry via `compose_registry![swissarmyhammer_commands, swissarmyhammer_kanban]` (focus added in the next task).
- [ ] `kanban-cli` constructs its registry via the same macro with the appropriate crate list.
- [ ] `swissarmyhammer-kanban::default_commands_registry` and friends are removed; no caller remains.
- [ ] Aggregation lives at the app layer (kanban-app, kanban-cli) rather than in `swissarmyhammer-kanban`.
- [ ] Behavior unchanged: every command id that was in the registry before this refactor is still in the registry afterward (verified by snapshot test).

## Tests

- [ ] New unit test in `swissarmyhammer-commands/src/registry.rs` (or a new `macros.rs` test file): `compose_registry_yields_concatenated_sources` — invoke the macro with two stub modules each exposing a `builtin_yaml_sources()` returning a known fixture; assert the resulting registry contains both sets of ids.
- [ ] New integration test in `kanban-app/tests/`: `command_id_set_unchanged_after_macro_refactor` — snapshot the set of command ids produced by the registry before and after this refactor. Capture the pre-refactor set first, then run the refactor and verify equality.
- [ ] Update `swissarmyhammer-kanban/tests/default_commands_registry.rs` to use the macro; rename if appropriate.
- [ ] Test command: `cargo nextest run -p swissarmyhammer-commands -p swissarmyhammer-kanban` — passes.

## Workflow

- Use `/tdd` — write the macro unit test first (will fail because the macro doesn't exist); add the macro; refactor the call sites; re-run. #nav-jump