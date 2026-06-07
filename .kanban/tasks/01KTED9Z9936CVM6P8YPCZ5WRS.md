---
position_column: todo
position_ordinal: dd80
project: ui-command-cleanup
title: Card J — Multi-key chord support in the command-service keys schema; migrate SEQUENCE_TABLES
---
## What
Independent, larger schema extension. The command-service `keys` schema today is a single string per keymap (vim/cua/emacs) and CANNOT express multi-key chords. The vim chords in `apps/kanban-app/ui/src/lib/keybindings.ts` `SEQUENCE_TABLES` (gg, dd, gt, zo → command ids) therefore live in the UI as the one justified exception. This card makes chords first-class in the catalogue so those bindings live in plugins like every other key.

Approach:
- Extend the command `keys` schema to allow a chord (sequence of keystrokes) per keymap, not just a single string. This is a real schema change spanning: the Rust command metadata (`RegisterCommand`/`CommandMetadata`/`CommandDef` keys representation), the plugin SDK TS types for `keys`, and the frontend keybinding handler so `createKeyHandler`/`extractKeymapBindings` build a chord-aware dispatch (the SEQUENCE_TABLES matching logic moves behind the schema).
- Migrate `SEQUENCE_TABLES` (gg/dd/gt/zo) into the owning plugins' command `keys` as chords, and delete the static table. Until this card lands, SEQUENCE_TABLES is the documented, justified UI exception (add a comment there pointing at this card).
- Keep single-key `keys` backward compatible (a chord of length 1 == today's behavior).

## Acceptance Criteria
- [ ] The `keys` schema (Rust metadata + SDK TS types + frontend handler) supports multi-key chord sequences per keymap, single-key still works.
- [ ] gg/dd/gt/zo are expressed as chords in their owning plugin command defs; `SEQUENCE_TABLES` is deleted from keybindings.ts.
- [ ] Chord dispatch resolves to the right command id under the real scope chain.

## Tests
- [ ] Rust: command-service test that a command can register a multi-key chord and it round-trips through metadata → CommandDef.
- [ ] UI: keybindings test (extend the keybindings.ts test suite) that typing `g g` resolves the plugin command via chord matching, single keys unaffected, and chord-prefix timeouts behave.
- [ ] Plugin e2e: a plugin command with a chord key registers and surfaces the chord.
- [ ] Relevant cargo + vitest suites green.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.