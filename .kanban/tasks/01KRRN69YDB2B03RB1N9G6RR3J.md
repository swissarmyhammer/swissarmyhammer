---
assignees:
- claude-code
depends_on:
- 01KRRN5X6MWQ157CC9PN2QZHWT
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff980
project: ai-panel
title: AI panel command scope and keybindings
---
## What
Make the AI panel a first-class citizen of the command system (see `ideas/kanban/app-architecture.md`).

- Register an AI panel command scope at the window layer:
  - `ai.toggle` — show/hide the panel (drives `AiPanelContainer` open-state)
  - `ai.focus` — move focus into the panel
  - `ai.newChat` — start a fresh stateless ACP session, clearing the conversation
  - `ai.model` — change model (`:ai model <name>`, autocomplete from `ai_list_models`)
  - `ai.cancel` — stop generation (`cancel` on the ACP client; available only while streaming)
- Add the command definitions to the appropriate builtin command YAML and the command implementations, following the existing pattern (`swissarmyhammer-commands` / `swissarmyhammer-kanban`, `compose_registry!`).
- Add keybindings for the vim / cua / emacs keymaps, consistent with the rest of the app.

## Acceptance Criteria
- [x] `ai.toggle`, `ai.focus`, `ai.newChat`, `ai.model`, `ai.cancel` are registered and resolve through the scope chain at the window layer.
- [x] The commands appear in the command palette with keybindings per keymap mode.
- [x] `ai.toggle` shows/hides the panel; `ai.newChat` starts a fresh session; `ai.cancel` is available only while streaming.
- [x] `cargo build` / `npm run build` succeed.

## Tests
- [x] Command-resolution tests: each `ai.*` command resolves to its handler from the window scope.
- [x] Test `ai.toggle` flips panel open-state and `ai.newChat` resets the conversation/session.
- [x] Test `ai.cancel` is unavailable when idle, available while streaming.
- [x] Relevant `cargo test` / `npm test` suites are green.

## Workflow
- Use `/tdd` — write the command-resolution and behavior tests first.

## Implementation Notes

### Design — backend YAML+impl + frontend-local execute
The AI panel's open-state, conversation, and ACP session are entirely webview-local (per the `AiPanelContainer` task `01KRRN5X6MWQ157CC9PN2QZHWT`: `localStorage` per board / `useConversation`; there is no backend store). So the five `ai.*` commands follow the established `ui.entity.startRename` pattern: a backend YAML definition + a no-op backend `Command` impl exist purely so the command is in the registry (palette visibility, keybinding hints, the YAML↔Rust completeness guard), and the **frontend resolves a local `execute` handler** before any dispatch reaches the backend.

### Where the YAML lives
`crates/swissarmyhammer-kanban/builtin/commands/ai.yaml` (new) — `ai` is a kanban-domain concept, so it joins the other kanban builtin command files (`task`, `view`, `perspective`, …) contributed via `swissarmyhammer_kanban::builtin_yaml_sources()` and composed at the app layer with `compose_registry!`. The five definitions carry `keys` blocks (vim/cua/emacs) and `ai.model` declares a `model` param (`from: args`, `shape: text`). No `menu` placement — the native menu builder only wires App/File/Edit/Navigation/Window, so a `View`-menu entry would be dead config; the commands surface via palette + keybindings (matching `ui.entity.startRename`).

### Backend command implementations / scope registration
`crates/swissarmyhammer-kanban/src/commands/ai_commands.rs` (new) — `AiToggleCmd`, `AiFocusCmd`, `AiNewChatCmd`, `AiModelCmd`, `AiCancelCmd`. Every `execute` is a deliberate no-op returning `Value::Null` (the webview intercepts). Registered via a new `register_ai()` helper in `commands/mod.rs`, called from `register_commands()` (62→67). Window-layer placement: the frontend registers them in `AppShell`'s global `CommandScopeProvider` — the window-layer command scope, an ancestor of every focused element — so they resolve through the scope chain app-wide.

### `ai.cancel` availability gating
Backend: `AiCancelCmd::available()` reads a new transient `UIState::ai_streaming` flag (`set_ai_streaming` / `ai_streaming`, `#[serde(skip)]` — the exact `can_undo` precedent), so `commands_for_scope` filters `ai.cancel` out of the palette when the conversation is idle. The webview pushes the flag via a new plain Tauri command `ai_set_streaming` (in `apps/kanban-app/src/ai/models.rs`) — transient availability-cache plumbing, like `set_undo_redo_state`, not an entity mutation. Frontend: `buildAiCommands(streaming)` sets `CommandDef.available = streaming` on `ai.cancel`, which both hides it from the palette (`collectAvailableCommands`) and makes its keybinding a no-op (`resolveCommand` returns null on a blocked command).

### How the backend command reaches the webview AI panel state
A module-level registry `apps/kanban-app/ui/src/ai/commands.ts` (new) bridges the window-layer commands to the AI panel subtree (the `triggerStartRename` pattern). `AiPanelContainer` registers `toggle`/`focus`/`setModel`; `AiPanelConversation` registers `newChat`/`cancel` and reports the ACP turn `status` via `setAiStreaming`. `AppShell`'s `buildAiCommands` `execute` closures call `triggerAi*`. `AppShell` subscribes to the streaming flag with `useSyncExternalStore(subscribeAiStreaming, …)` so `ai.cancel` is rebuilt with the fresh `available` whenever a turn starts/ends. The Container (not the pure-View `AiPanel`) also `invoke`s `ai_set_streaming` so the Container/View split is preserved. `ai.focus` expands a collapsed panel then focuses the prompt `<textarea>`.

### Keybindings (vim / cua / emacs — identical across all three)
`ai.toggle` = `Mod+J`, `ai.focus` = `Mod+I`, `ai.newChat` = `Mod+Shift+J`, `ai.cancel` = `Mod+.`; `ai.model` has no key (it takes an arg). Verified no collisions with existing bindings. Declared in `ai.yaml` `keys` (uppercase, the YAML convention — feeds menu accelerators / palette hints) AND in `BINDING_TABLES` + the `buildAiCommands` `CommandDef.keys` (lowercase canonical form `normalizeKeyEvent` emits, e.g. `Mod+j` — matching `file.closeBoard`'s `Mod+w`). The `BINDING_TABLES` entries cover the no-focus case; the `CommandDef.keys` feed `extractScopeBindings` when the global scope is in the focused chain.

### Files changed
- `crates/swissarmyhammer-kanban/builtin/commands/ai.yaml` (new) — 5 command definitions.
- `crates/swissarmyhammer-kanban/src/commands/ai_commands.rs` (new) — 5 `Command` impls + unit tests.
- `crates/swissarmyhammer-kanban/src/commands/mod.rs` — `register_ai()`; count test 62→67; `ai.*` resolution/availability/no-op tests.
- `crates/swissarmyhammer-kanban/src/lib.rs` — `builtin_yaml_sources` test includes `ai`.
- `crates/swissarmyhammer-commands/src/ui_state.rs` — transient `ai_streaming` field + `ai_streaming()`/`set_ai_streaming()` + tests.
- `crates/swissarmyhammer-kanban/tests/builtin_commands.rs` — 5 `ai.*` ids; counts 29→34, 71→76.
- `crates/swissarmyhammer-kanban/tests/composed_commands_registry.rs` — count 71→76, id-set snapshot.
- `crates/swissarmyhammer-kanban/tests/snapshots/*_full.json` — regenerated (4 `ai.*` rows added; `ai.cancel` correctly absent at idle).
- `apps/kanban-app/src/ai/models.rs` — `ai_set_streaming` Tauri command.
- `apps/kanban-app/src/main.rs` — register `ai_set_streaming`.
- `apps/kanban-app/ui/src/ai/commands.ts` (new) + `commands.test.ts` (new) — the AI command registry.
- `apps/kanban-app/ui/src/components/app-shell.tsx` — `buildAiCommands`, `useAiStreaming`, wired into `globalCommands`.
- `apps/kanban-app/ui/src/components/app-shell.ai-commands.test.tsx` (new) — window-layer `ai.*` resolution + `ai.cancel` gating.
- `apps/kanban-app/ui/src/components/ai-panel-container.tsx` + `.test.tsx` — register `toggle`/`focus`/`setModel`, backend streaming sync; `ai.toggle`/`ai.focus` behavior tests.
- `apps/kanban-app/ui/src/components/ai-panel.tsx` + `.test.tsx` — register `newChat`/`cancel`, report streaming; `ai.newChat`/`ai.cancel` behavior tests.
- `apps/kanban-app/ui/src/lib/keybindings.ts` + `.test.ts` — `ai.*` `BINDING_TABLES` entries (all 3 keymaps) + parity test.

### Verification (actual command output)
- `cargo build --workspace`: Finished, clean.
- `cargo clippy -p swissarmyhammer-kanban -p swissarmyhammer-commands -p kanban-app --all-targets -- -D warnings`: Finished, 0 warnings.
- `cargo test -p swissarmyhammer-commands -p swissarmyhammer-kanban`: all `0 failed` (incl. new `ai_commands` / `ui_state` / registry tests).
- `cargo test -p kanban-app`: `0 failed`.
- `npm run build` (`apps/kanban-app/ui`): built, clean.
- `npm test` (`apps/kanban-app/ui`): 2243 passed, 35 skipped; the new `ai.*` suites green. The 3 failures (4 files) are the pre-existing, task-excluded stale-fixture suites (`slugify.parity.node.test.ts`, `editor-save.test.tsx`, `board-integration.browser.test.tsx` — `01KRS426Q36ZN3DYBX2S0AS82T`) and the CodeBlock/Shiki flake (`01KRVG4QSXPQ2FW5SG61M8EHAP`).
- `git status` clean of pollution — only the intended source/test/snapshot files plus the 5 new files.

## Review Findings (2026-05-18 10:45)

### Warnings
- [x] `crates/swissarmyhammer-kanban/builtin/commands/ai.yaml:43-49` — `ai.model` declares a `model` param with `from: args, shape: text` but ships no `options_from` resolver, so the command's "What" criterion ("autocomplete from `ai_list_models`") is unmet. The command is functional — the palette renders a `<CommandPopover>` text input for the `shape: text` param (the same path `perspective.save`'s `name` uses) — but the user must type a raw model id (`claude-code`, `qwen-coder`) into a free-text box with no completion. The codebase already has the mechanism for this (`options_from`, e.g. `perspective.sort.set`'s `field`/`direction` params at `perspective.yaml:185,244,248`): add an `options_from: "ai.models"` (or similar) resolver backed by `ai_list_models` so the popover offers the configured models. The five Acceptance-Criteria / Tests checkboxes are all satisfied; this gap is against the "What" prose only, hence a warning rather than a blocker.

## Review Finding Resolution (2026-05-18)

Wired the `ai.model` model picker through the existing `options_from` resolver mechanism — the exact pattern `perspective.sort.set`'s `field` param uses with `perspective.fields`.

### The `options_from` mechanism, end to end
- **YAML** (`ai.yaml`): `ai.model`'s `model` param changed from `from: args, shape: text` to `from: args, shape: enum, options_from: "ai.models"`. `shape: enum` makes the palette render a `<CommandPopover>` picker (not a free-text box); `options_from` names the backend resolver.
- **Resolver** (`crates/swissarmyhammer-kanban/src/commands/options_resolvers.rs`): new `AiModelsResolver` (key `ai.models`), data-free, registered via the new `register_kanban_resolvers()` and added to `default_options_registry()` (kanban previously owned zero resolvers). It mirrors `swissarmyhammer_perspectives::PerspectiveFieldsResolver` exactly: it downcasts `OptionsContext.data` to `&OptionsSources`, pulls a new `AiOptionsData { models: Vec<AiModelInfo> }`, and projects each `AiModelInfo { id, label }` onto a `ParamOption { value: id, label }`.
- **Why a consumer-supplied-data resolver, not a static one** (`view.kinds` / `sort.directions` precedent rejected): the AI model set is a *runtime* enumeration driven by `swissarmyhammer-config`'s `ModelManager` (filesystem agent discovery) + `which`-based Claude CLI detection. `swissarmyhammer-kanban` is a pure-domain crate and intentionally does not depend on `swissarmyhammer-config`, so the resolver cannot enumerate models itself. Following the `PerspectiveFieldsResolver` pattern, the resolver is data-free and the consumer threads the model list in via `OptionsSources` — which is exactly why the kanban crate stays correctly layered (`ARCHITECTURE.md` tier rule).
- **Plumbing** (`scope_commands.rs` / `dynamic_sources.rs`): `DynamicSources` gained an `ai_models: Vec<AiModelInfo>` field (consumer-supplied runtime data, like `windows`); `build_options_sources` inserts `AiOptionsData` from it; `DynamicSourcesInputs` gained the matching `ai_models` input.
- **Consumer** (`apps/kanban-app/src/commands.rs`): `build_dynamic_sources` now calls a new `gather_ai_models()` helper that invokes `ai_list_models()` (the GUI-side enumeration in `apps/kanban-app/src/ai/models.rs`) and maps `Model { id, label, .. }` → `AiModelInfo { id, label }`. On enumeration failure it logs and falls back to an empty list (graceful degradation the resolver already tolerates).
- **Frontend**: zero changes needed — the palette consumes `ResolvedCommand.params[].options` for any enum param through the same `<CommandPopover>` path `perspective.group` / `perspective.sort.set` already use, and `buildAiCommands`'s `ai.model` `execute` closure already reads `opts.args.model`.

### Files changed (resolution)
- `crates/swissarmyhammer-kanban/builtin/commands/ai.yaml` — `model` param → `shape: enum` + `options_from: "ai.models"`.
- `crates/swissarmyhammer-kanban/src/commands/options_resolvers.rs` — `AiModelInfo`, `AiOptionsData`, `AiModelsResolver`, `register_kanban_resolvers()`; `default_options_registry()` registers `ai.models`; 6 new unit tests.
- `crates/swissarmyhammer-kanban/src/scope_commands.rs` — `DynamicSources.ai_models` field; `build_options_sources` inserts `AiOptionsData`; 11 spelled-out test constructors converted to `..Default::default()`.
- `crates/swissarmyhammer-kanban/src/dynamic_sources.rs` — `DynamicSourcesInputs.ai_models` input, threaded into `build_dynamic_sources`.
- `apps/kanban-app/src/commands.rs` — `gather_ai_models()` helper; `build_dynamic_sources` threads it into `DynamicSourcesInputs`.
- `crates/swissarmyhammer-kanban/tests/options_enrichment.rs` — 2 new end-to-end tests (`ai_model_command_carries_model_options_from_resolver` asserts the real `ai.yaml` `ai.model`'s `model` param is `shape: enum` + `options_from: ai.models` and is populated with the supplied models; `ai_model_command_resolves_to_empty_options_when_no_models_configured` pins the empty-answer contract). Modeled on `perspective_sort_set_command_carries_field_and_direction_options`.
- `crates/swissarmyhammer-kanban/tests/dynamic_sources_headless.rs`, `tests/perspective_migration.rs` — `ai_models: vec![]` added to `DynamicSourcesInputs` constructors.

### Verification (resolution — actual command output)
- `cargo build` (workspace): Finished, clean.
- `cargo clippy -p swissarmyhammer-kanban -p kanban-app --all-targets -- -D warnings`: Finished, 0 warnings.
- `cargo test -p swissarmyhammer-kanban --lib`: 1143 passed, 0 failed (incl. 6 new `options_resolvers` tests).
- `cargo test -p swissarmyhammer-kanban` (options_enrichment, command_snapshots, builtin_commands, composed_commands_registry, dynamic_sources_headless, perspective_migration): all `0 failed` — `options_enrichment` 13 passed (incl. 2 new `ai.model` tests), `command_snapshots` 14 passed (no churn — snapshots serialize `id/name/keys/...`, not `params`).
- `cargo test -p kanban-app`: 0 failed.
- `npm run build` (`apps/kanban-app/ui`): built, clean.
- `npx vitest run` for `ai/commands.test.ts`, `app-shell.ai-commands.test.tsx`, `command-palette.test.tsx`, `command-popover.test.tsx`: 56 passed, 0 failed.