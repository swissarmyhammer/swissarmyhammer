---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd680
title: Reconcile JS nav port with kernel + cardinal nav lands on the card (option B) — DONE
---
RESOLVED (commit 69114207f, pushed). Two things, one change set:

1. Cardinal nav now lands on the CARD across columns (option B). Kernel navigate.rs: replaced the sibling-exclusion pass with an ancestry-aware tie-break in better_candidate — on a beam-score TIE (a card and its inner inspect-button/title share an edge → equal score), the enclosing CARD wins over the focusable leaf inside it. Non-focusable zones (columns, board well, panel/view-area wrappers) were already skipped as candidates. Result: Up/Down between cards AND Left/Right across columns land on the card, never its field/button; cross-AREA moves (composer↔view) still work; a move never stops on an indicator-less zone.

2. JS port reconciled with the kernel (the original follow-up). spatial-shadow-registry.ts::navigateInShadow rewritten from the stale sibling-cascade (which referenced deleted kernel fns) to faithfully mirror geometric_pick: focusable-skip + ancestor-skip + half-plane/in-beam + ancestry tie-break + per-direction override. focusable threaded through ShadowEntry / RegistrationRecord / the setup.ts register-hook. UI spatial tests now exercise the real kernel algorithm, not a phantom one.

Verification:
- cargo test -p swissarmyhammer-focus: 66 lib (incl new left_crosses_to_card_not_its_tied_inner_button) + all integration green.
- tsc --noEmit: clean.
- vitest spatial suite: 47/49 files pass; 15 failures are ALL pre-existing in ai-panel.spatial (5) + ai-panel-elicitation.spatial (10) — jump-to/Enter/ACP-timeout, NOT nav (matches the committed baseline exactly → zero nav regressions).
- The ai-panel cross-zone test's `ui:view-area` placeholder is now `showFocus` (focusable) — under the geometric kernel a move lands on a real target, not a showFocus=false zone.

Pre-existing, separate (not this task): the 15 AI-panel jump-to/Enter/ACP-timeout failures — needs its own triage.

Needs user verification in the real app: Up/Down card↔card; Left/Right cross-column → the aligned card (not its inspect-button, not the column); inside a card up/down→fields; composer ↔ view cross-area.

See memory project_nav_js_port_diverged_from_kernel.

#spatial-nav #navigation #testing