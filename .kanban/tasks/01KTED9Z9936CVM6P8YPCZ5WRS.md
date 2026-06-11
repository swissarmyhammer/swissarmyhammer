---
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9d80
project: ui-command-cleanup
title: Card J — Multi-key chord support in the command-service keys schema; migrate SEQUENCE_TABLES
---
## What
Independent, larger schema extension. The command-service `keys` schema today is a single string per keymap (vim/cua/emacs) and CANNOT express multi-key chords. The vim chords in `apps/kanban-app/ui/src/lib/keybindings.ts` `SEQUENCE_TABLES` (gg, dd, gt, zo → command ids) therefore live in the UI as the one justified exception. This card makes chords first-class in the catalogue so those bindings live in plugins like every other key.

Approach:
- Extend the command `keys` schema to allow a chord (sequence of keystrokes) per keymap, not just a single string. This is a real schema change spanning: the Rust command metadata (`RegisterCommand`/`CommandMetadata`/`CommandDef` keys representation), the plugin SDK TS types for `keys`, and the frontend keybinding handler so `createKeyHandler`/`extractKeymapBindings` build a chord-aware dispatch (the SEQUENCE_TABLES matching logic moves behind the schema).
- Migrate `SEQUENCE_TABLES` (gg/dd/gt/zo) into the owning plugins' command `keys` as chords, and delete the static table. Until this card lands, SEQUENCE_TABLES is the documented, justified UI exception (add a comment there pointing at this card).
- Keep single-key `keys` backward compatible (a chord of length 1 == today's behavior).

## Implementation notes (as landed)
- **Chord schema**: a `keys` value is one or more canonical keystrokes separated by single spaces (VS Code-style): `"g g"`, `"g Shift+T"`, `"d d"`. A single keystroke is a chord of length 1 — wire shape stays `Record<keymap, string>`, so `CommandMetadata` mirrors, `KeysDef`, and the native-menu layer keep their types; the schema change is the chord *grammar*, enforced at registration (`is_valid_chord` in the command service, structured `InvalidKeyBinding` rejection) and interpreted by the webview chord state machine in `createKeyHandler` (pending buffer, 500ms-per-step timeout, chord-prefix-beats-single-key precedence, abandoned-prefix fallback to the terminating key).
- **zo caveat**: `task.toggleCollapse` is registered NOWHERE (no plugin/YAML/bus handler/CommandDef) — the `z o` chord dispatched a nonexistent id. It was dropped, not migrated; follow-up card `01KTWAM76RMH9G8D8VY5GZPN62` documents the decision and the path to a real owner if collapse-by-key is wanted. gg/gt/gT/dd migrated to nav-commands / perspective-commands / entity-commands as chords.
- Native menu (`menu.rs`): chords are never accelerator-expressible; `resolve_accelerator` now skips a chord mode-binding and falls back to cua (nav.first keeps `Home` in vim mode).

## Acceptance Criteria
- [x] The `keys` schema (Rust metadata + SDK TS types + frontend handler) supports multi-key chord sequences per keymap, single-key still works.
- [x] gg/dd/gt/zo are expressed as chords in their owning plugin command defs; `SEQUENCE_TABLES` is deleted from keybindings.ts. (zo had NO owning command anywhere — dropped with follow-up card `01KTWAM76RMH9G8D8VY5GZPN62`.)
- [x] Chord dispatch resolves to the right command id under the real scope chain.

## Tests
- [x] Rust: command-service test that a command can register a multi-key chord and it round-trips through metadata → CommandDef. (`tests/chord_keys.rs` — round-trip via `list command` + validation rejections.)
- [x] UI: keybindings test (extend the keybindings.ts test suite) that typing `g g` resolves the plugin command via chord matching, single keys unaffected, and chord-prefix timeouts behave.
- [x] Plugin e2e: a plugin command with a chord key registers and surfaces the chord. (`builtin_nav_commands_e2e` / `builtin_perspective_commands_e2e` / `builtin_entity_commands_e2e` pin `"g g"` / `"g t"` / `"g Shift+T"` / `"d d"` from real plugin registration.)
- [x] Relevant cargo + vitest suites green. (cargo nextest -p swissarmyhammer-command-service: 136 passed; vitest keybindings + mirror + inspectable: 119 passed; tsc clean.)

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.

## Review Findings (2026-06-11 17:05)

Verified: state machine edge cases (miss re-resolves terminating key INCLUDING fresh-prefix detection; buffered prefix not preventDefaulted = exact legacy SEQUENCE_TABLES parity, confirmed at HEAD; editable targets bail before chord logic; bindings re-merged per keydown so no stale table mid-chord). menu.rs compile-proxied via LSP diagnostics (LiveLsp, 0 errors/0 warnings); `resolve_accelerator` chord-skip → cua fallback and the post-trim whitespace rejection are sound and pinned. `task.toggleCollapse` confirmed dead (no registration anywhere; legacy `z o` dispatched a nonexistent id); follow-up card `01KTWAM76RMH9G8D8VY5GZPN62` exists. SEQUENCE_TABLES symbol deleted (comments only). Re-runs: cargo nextest 136/136; scoped vitest 143/143; tsc clean. Red-green probe: inverting prefix-before-exact precedence fails exactly 2 tests ("a chord prefix shadows a single-key binding on the same key", "a timed-out prefix never fires its single-key shadow late"); restored green. Pre-existing-failure claim substance verified by isolation (entity-card + board-view failures reproduce identically with HEAD's keybindings.ts), though a full vitest run shows 54 pre-existing failures across ~10 files, not the claimed 18 in 4.

### Warnings
- [x] `apps/kanban-app/ui/src/lib/keybindings.test.ts` — no test covers a missed chord prefix falling through into a NEW chord prefix: pending `"g"`, next key starts a different chord (real with the shipped catalogue: `g` then `d d` must still fire `entity.archive`). The existing miss-path tests compose only with an exact match (`g` → `u`) or a no-binding key (`g` → `x`), so a regression that dropped the prefix re-check in the bare `resolve(normalized)` fallback would pass every current test while silently killing `d d` after an abandoned `g`. Add one test: `g`, `d`, `d` → `entity.archive`.
  - Done: added "an abandoned prefix falls through into a NEW chord prefix (g, d, d)" to the chord suite; passes on current code (coverage pin), and a red-check (temporarily dropping the prefix re-check in the bare fallback) fails exactly this one test, confirming it pins the gap. 103/103 keybindings tests green, tsc clean.

### Nits
- [x] `apps/kanban-app/ui/src/lib/keybindings.ts` — the buffered prefix key is not `preventDefault()`ed (correct legacy parity, documented). Harmless for the current `g`/`d` chord roots, but a future chord rooted at a key with a browser default (`Space`, `Tab`) would leak that default while buffered, and registration validation cannot catch it. Worth one sentence in the `createKeyHandler` chord docs warning chord authors away from default-bearing root keys.
  - Done: extended the Prefix bullet in the `createKeyHandler` chord-resolution docs with a caveat warning chord authors away from default-bearing root keys (Space scrolls, Tab moves focus); comment only, no behavior change.