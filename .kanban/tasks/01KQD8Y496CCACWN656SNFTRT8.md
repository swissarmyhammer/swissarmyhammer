---
assignees:
- claude-code
depends_on:
- 01KQD8X3PYXQAJN593HR11T7R4
- 01KQD8XM2T0FWHXANCK0KVDJH1
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff817f80
project: spatial-nav
title: 'Path monikers Layer 3: Manual log verification in npm run tauri dev (mandatory acceptance gate)'
---
## Subset of `01KQD6064G1C1RAXDFPJVT1F46`

Third and final sub-task. Depends on Layers 1 and 2 landing first.

## What

The parent card's Layer 3 plan is **mandatory** — the past two passes on this surface had passing simulator tests but broken production. The simulator does not model the duplicate-moniker bug.

### Steps

Run `npm run tauri dev`:

- [ ] Open an inspector on a task. `log show --last 1m --predicate 'subsystem == "com.swissarmyhammer.kanban"' --info --debug | grep duplicate` — assert zero output.
- [ ] Click a field in the inspector. Press ArrowDown. Same log query — zero duplicate warnings AND subsequent `ui.setFocus` `scope_chain` log lines contain only paths starting with `/window/inspector/...`.
- [ ] Press ArrowDown at the last field. Focus echoes (stays put).
- [ ] Press ArrowUp inside inspector — same scope-chain check, no `card:*` / `column:*` paths leak in.
- [ ] Press Escape. Inspector closes. Focus restores via `last_focused` on the originating card.

Capture the log output as task-attached evidence (paste relevant lines into a comment) before marking this card done.

## Acceptance Criteria

- [ ] No `duplicate moniker registered against two distinct keys` warning when an inspector opens.
- [ ] ArrowDown / ArrowUp inside an inspector stays inside `/window/inspector/...`. No `card:*` / `column:*` paths in `ui.setFocus` `scope_chain` log lines.
- [ ] ArrowDown at the last field echoes (focus stays put).
- [ ] Escape closes the inspector and restores focus to the originating card via `last_focused`.

## Depends on

- Layer 1 (kernel) and Layer 2 (Tauri + UI) sub-tasks of the parent card.

## Related

- Parent: `01KQD6064G1C1RAXDFPJVT1F46`
