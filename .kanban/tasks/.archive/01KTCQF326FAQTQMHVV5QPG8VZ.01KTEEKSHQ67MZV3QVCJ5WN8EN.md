---
assignees:
- claude-code
position_column: todo
position_ordinal: cf80
title: Palette won't open / not on OS menu — SUBSUMED by Card A + CRQ6KJ (recommend close)
---
## RECONCILED 2026-06-06 (dedup sweep)
This card's title ("ui-state-changed must use emit_to") no longer matches its body. Its actual content — "why does the palette not open: hotkey path + missing OS-menu affordance" — is fully covered by two canonical cards:
- **Card A `01KTCQFH7AEQDZD0QETSMCMGP0`** — palette opener (+ nav.*) on the OS menu, built FROM the CommandService catalogue.
- **Palette bug `01KTCRQ6KJ67FJWYEZFQ6J7R13`** — the hotkey/execution open path (the command's `keys` drive the live keymap, a single service owns `palette_open`, single TS plugin command, no Rust impl).

The `emit_to(window_label)` hypothesis in the original title was tried and reverted (owner note below). **Recommend CLOSING this card as subsumed.** Kept (not deleted) because it is owner-authored — surfacing rather than removing. Any unique signal is preserved below.

---

REOPENED 2026-06-06 — prior fix did NOT solve the problem and was discarded.

## OWNER CORRECTION
The command palette STILL DOES NOT OPEN after the per-window `emit_to` change. So `emit_to(window_label)` was NOT the (whole) root cause of the palette/inspector not working. The code (the `UiStateEventSink` seam + tests, and the pre-existing emit_to partial) was reverted to HEAD.

Additionally: the command palette opener is MISSING FROM THE OS MENU entirely (see the broader OS-menu effort / card 01KTCQFH7AEQDZD0QETSMCMGP0 and #bug card 01KTCRQ6KJ67FJWYEZFQ6J7R13).

## Next step
Do NOT assume `emit_to` is the fix. Re-investigate from scratch WHY the command palette does not open — both the hotkey path AND why there is no OS-menu affordance to open it. Likely shares a root cause with the navigation-OS-menu command-surfacing problem (commands not reaching the surface that should present them). Focus the OS-menu work first per the owner. TDD, RED first.