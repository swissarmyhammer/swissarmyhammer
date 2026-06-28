---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8a80
title: Render command caption templates ({{entity.type}}) against the focused object — no raw placeholders in palette/menus
---
## What

LIVE BUG (user-observed): the command popup (palette) shows captions like `Inspect {{entity.type}}` — the raw template placeholder, unrendered. Command names/captions declared in builtin plugins may contain `{{...}}` placeholders that must be rendered against the FOCUSED OBJECT (the entity at the focused scope chain) before display — in the palette, the OS menu, context menus, and anywhere else captions surface.

**Design directive from the user**: render these "in a thoughtful way based on the focused object — not hard coded in the plugins." The template in the plugin is the declaration; rendering is the system's job, driven by focus context.

## Resolved design (implemented)

**Placeholder inventory** (grep `\{\{` across `builtin/plugins/**`): exactly one placeholder key, `{{entity.type}}`, in 7 captions — `ui-commands` (`ui.inspect` "Inspect …") and `entity-commands` (`entity.delete/archive/unarchive/cut/copy/paste`).

**Engine**: minimal token-scanning resolver in `crates/swissarmyhammer-command-service/src/caption.rs` (`render_caption`), NOT Liquid — pulling the Liquid engine into the command service for `{{a.b}}` substitution is a heavyweight new dependency edge; same judgment the legacy `swissarmyhammer-kanban::scope_commands::resolve_name_template` made, generalized with token scanning so whitespace-padded (`{{ entity.type }}`), unknown, and malformed tokens all degrade cleanly. Documented in the module docs.

**Render-time context**: `ListCommand` gained `ctx: CommandContext` (`#[serde(default)]`, same shape as execute/available). `handle_list` renders `name` and `menu_name` of every listed command through `render_caption` — the response only ever carries display-ready strings. Entity type resolves from the explicit `ctx.target` moniker when present (context-menu semantics), else the innermost scope-chain moniker (palette semantics); type token before the first colon, display-cased per `_`/`-` word ("task" → "Task", "saved_search" → "Saved Search").

**Fallback semantics**: unresolvable placeholder (unknown key, no entity context) renders empty and whitespace is tidied — "Inspect {{entity.type}}" with no context → "Inspect". Unclosed `{{` drops the malformed remainder. A rendered caption NEVER contains `{{`.

**Surfaces**:
- Palette: `command-palette.tsx` forwards the focused scope chain via `useCommandList({scopeChain})` → `list command` `ctx.scope_chain`. Zero template logic in React.
- OS menu: existing mechanism kept. `menu.rs::collect_menu_entries` (initial build, no focus context) and `apply_menu_item_state`'s unavailable-command branch now use the shared `render_caption` with the empty context (generic fallback) instead of the previous hardcoded ` {{entity.type}}` strip; the existing focus-driven refresh continues to set scope-resolved names. No new refresh system.
- Context menu (`context-menu.ts`): fetches via `useCommandList()` (no ctx) — server now returns the clean generic fallback ("Delete", never "Delete {{entity.type}}"). Per-entity rendering at right-click time ("Delete Task") remains for card 01KTCS1X9A7B5HT3H0GEDZJQ8R; the mechanism (pass `ctx.target`/`ctx.scope_chain` at list time) is in place.

## Acceptance Criteria
- [x] Command palette shows `Inspect Task` (or the focused entity's proper type name) when a task is focused — never `{{entity.type}}`
- [x] All other placeholder-bearing captions found in builtin/plugins render correctly in the palette (single mechanism covers all 7; guard sweep proves it)
- [x] OS menu / context menus never display raw `{{...}}` (rendered with context where available, generic fallback where not)
- [x] Rendering happens in the Rust command service driven by the scope chain — zero template logic in React; UI receives display-ready strings
- [x] Plugins keep their declarative templates — no captions hardcoded per entity type in plugin or UI code

## Tests
- [x] Rust unit tests for the caption renderer (10 in `caption.rs`): scope-chain resolution, target precedence, unknown placeholder → defined fallback never raw, empty context, whitespace-in-braces, malformed token, multi-word casing, path-shaped moniker ids
- [x] Integration tests through the real command-service list path (`tests/list_renders_captions.rs`, 6 tests): `list command` with a focused task scope chain returns `Inspect Task`; no-ctx fallback; target precedence; menu_name rendering; never-raw guarantee. Written RED-first (all 6 failed with the raw template before implementation).
- [x] Guard test sweeping all 9 registered builtin plugins (`full_baseline_e2e.rs::no_surfaced_display_caption_contains_raw_placeholders`): no listed `name`/`menu_name` contains `{{`, with and without ctx
- [x] `cargo nextest run -p swissarmyhammer-command-service` — 123/123 passed (the previously-noted keybinding e2e failures did not reproduce)
- [x] Touched vitest green: use-command-list (7), command-palette (incl. updated scope-sourcing test asserting scopeChain forwarding), command-palette.availability — 53/53; `tsc --noEmit` clean. New scopeChain-wiring vitest verified red-green-red.

## Constraints
- NO whole-workspace cargo build/clippy/run — `tauri dev` hot-reloads; crate-scoped nextest only. (Honored: kanban-app crate NOT compiled; `menu.rs` edits made by reading — they compile on the next app rebuild.)
- Build on the committed state (4a3a2c780 / 541e85ce3); `.kanban/actors/wballard.jsonl` is in UU conflict — never touch it. (Honored.)

## Workflow
- Use `/tdd` — failing test first (list command with focused scope chain must return rendered caption), then implement. (Done: 6 integration tests RED → renderer + ctx wiring → GREEN.)

## Review Findings (2026-06-10 09:25)

Verified during review: 123/123 `cargo nextest run -p swissarmyhammer-command-service`; red-green-red probe (identity-stubbed `render_caption` → all 6 `list_renders_captions` tests failed with the raw template, restored → 123/123 again); scoped vitest 53/53 (use-command-list, command-palette, command-palette.availability, context-menu); `tsc --noEmit` clean; no template logic in React (`{{` matches in ui/src are JSX props and test fixture names only); palette forwards innermost-first `scopeChain`, context-menu and menu use the generic fallback; `menu.rs` imports (`render_caption`, `CommandContext`) resolve against the crate's public exports and the existing Cargo dependency; the guard sweep boots all 9 builtin plugins and asserts `name`/`menu_name` are `{{`-free with and without ctx. The "keybinding e2e failures did not reproduce" claim is consistent with the earlier fix (01KTPDTH772HSEV5F7R1DKYDNJ).

### Warnings
- [x] `crates/swissarmyhammer-command-service/src/caption.rs:124` / `crates/swissarmyhammer-kanban/src/scope_commands.rs:170` — There are now two live resolvers for the same `{{entity.type}}` placeholder with divergent display-casing: `display_case` splits on `_`/`-` and uppercases each word ("saved_search" → "Saved Search"), while the legacy `resolve_name_template` only uppercases the first character ("saved_search" → "Saved_search"). The OS menu's focus-driven refresh writes labels from the legacy resolver while the palette shows the new renderer's output — identical today (all entity types are single-word) but they silently diverge the day a multi-word entity type ships, which is exactly the two-surfaces-disagree bug class this card fixed. `swissarmyhammer-kanban` already depends on `swissarmyhammer-command-service`, so the fix is cheap: export the entity-type display-casing helper from the caption module and use it in `resolve_name_template`'s `{{entity.type}}` arm (this also retires that arm's non-Unicode-safe `&entity_type[..1]` byte slice).

### Nits
- [x] `crates/swissarmyhammer-command-service/src/types.rs:175` — The `CommandContext::scope_chain` doc example reads `["board:01ABC", "task:42"]` (outermost-first), but the actual convention — which caption rendering now semantically depends on via `scope_chain.first()` = innermost — is innermost-first (`scopeChainFromScope` walks innermost → root). A backend caller following the stale example would get "Inspect Board" with a task focused. Flip the example to `["task:42", "board:01ABC"]` and state the ordering.
- [x] `apps/kanban-app/ui/src/hooks/use-command-list.ts:154` — `scopeChainKey = scopeChain?.join(...)`: monikers can contain spaces (path-shaped attachment ids, e.g. `attachment:/some path/p.png`), so two different chains can collide on the joined key and suppress a caption re-fetch on focus change. Join with a delimiter that cannot occur in a moniker.

## Rework (2026-06-10, second pass)

**Warning — one canonical `{{entity.type}}` resolver**: `display_case` in `caption.rs` is now `pub` (exported from `swissarmyhammer-command-service` lib.rs alongside `render_caption`, with docs naming it the ONE canonical casing rule). `resolve_name_template`'s `{{entity.type}}` arm in `scope_commands.rs` now calls `swissarmyhammer_command_service::display_case`, retiring the first-char-only logic and the non-Unicode-safe `&entity_type[..1]` byte slice (empty input handled by `display_case` itself). Single-word types unchanged ("task" → "Task"); multi-word snake/kebab types now title-case identically on every surface.

**Lockstep guard (TDD, RED-first)**: new `scope_commands::tests::entity_type_captions_match_the_shared_caption_renderer` pins that `resolve_name_template` and `render_caption` produce IDENTICAL captions for the same entity type across single-word, snake_case, and kebab-case inputs. Watched RED before the fix (`"Inspect Saved_search"` vs `"Inspect Saved Search"`), GREEN after.

**Nit (types.rs)**: `CommandContext::scope_chain` doc now states innermost-first ordering with example `["task:42", "board:01ABC"]` and notes that caption rendering's `scope_chain.first()` makes the ordering load-bearing.

**Nit (use-command-list.ts)**: the previous separator was actually a raw NUL byte embedded in the source (unambiguous, but it made the .ts file binary-detected — `grep` reported "Binary file matches", and the byte renders invisibly in review tooling, which is how it read as `join(" ")`). Replaced with `JSON.stringify(scopeChain)` — fully unambiguous AND plain UTF-8 text. New vitest pins the non-collision behavior (two chains a space-join would flatten identically still trigger a re-fetch); red-green-red probed by temporarily swapping in `join(" ")` (test failed) and restoring (8/8 pass).

**Verification**: `cargo nextest run -p swissarmyhammer-command-service` 123/123; `cargo nextest run -p swissarmyhammer-kanban` 1234 run: 1233 passed, 1 failed — the single failure is `filter_integration::s17_tag_names_with_special_chars`, a PRE-EXISTING regression at committed HEAD (deterministic 3/3, tag-parser slug charset reverted by merge 606685949; provably unrelated to caption casing). Filed as card 01KTRZFB7FA9THYRZ5PVVXA0GW. Scoped vitest `use-command-list.test.tsx` 8/8 (7 prior + new collision guard); `tsc --noEmit` clean. No whole-workspace build; kanban-app crate not compiled; `.kanban/actors/wballard.jsonl` untouched.