---
assignees:
- claude-code
attachments:
- 01KR6YRGB6EBY6JG57D31EBP0F-image-LEFRRLR3zJQ6haj77VrwDgQk1WwdxZ.png
- 01KR6YRV0CQ9DGY1W1QVA566Z8-01KR6YRT0S0RWPG8R7YFNGJT8W-image-j9jBODrxZuGJaiYxWB9gLwDmHggV83.png
position_column: todo
position_ordinal: 7c80
title: menu.rs resolve_accelerator drops valid named keys (Enter, Escape, ArrowUp, etc.)
---
## Symptom

Menu items for nav.* commands display no accelerator next to their label, even though the YAML provides bindings.

## Root cause

`resolve_accelerator` in `kanban-app/src/menu.rs` filters out keys where `len > 1 && !contains('+')`. The intent is to skip vim chord strings (`"dd"`, `":q"`), but the filter also rejects named keys: `Enter`, `Escape`, `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Home`, `End`, etc.

Result: 7 of the 8 nav.* commands have no rendered accelerator (only `nav.last`'s vim binding `Shift+G` survives, because the `+` keeps it past the filter).

## Fix direction

Replace the heuristic with an allowlist of known named keys. Tauri's accelerator format documents the canonical names — start from that list. Anything not matching `[a-z]` (single char) or a known named key or a `Mod+`/`Shift+` chord is the actual filter target.

## Reproduction

Open the app's native menu after this branch lands. Look at `Navigation > Drill In` — should show `Enter`, shows nothing.

## Acceptance Criteria

- [ ] Menu accelerators render for `nav.up`/`down`/`left`/`right` (Arrow keys).
- [ ] Menu accelerators render for `nav.drillIn` (Enter) and `nav.drillOut` (Escape).
- [ ] Menu accelerators render for `nav.first`/`last` per mode where they exist.
- [ ] Vim chord strings (e.g. `gg`) still get filtered out (no garbled accelerator labels).

## Tags

#bug