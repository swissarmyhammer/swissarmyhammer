---
comments:
- actor: claude-code
  id: 01kvqx1svpbx1dsvhe9m1rtsb4
  text: |-
    Picked up. Research done. Key findings:
    - Header collapse button: in AiPanel body inside AiPanelFocusScope zone moniker="ui:ai-panel" -> wrapping in AiPanelPressable moniker "ui:ai-panel.collapse" yields FQM /window/ui:ai-panel/ui:ai-panel.collapse. Covered by existing renderPanel harness in ai-panel.spatial.test.tsx.
    - Rail expand button: lives in ai-panel-container.tsx (AiPanelContainer), which IS inside <FocusLayer name="window"> in App.tsx (lines 103-126). So AiPanelPressable will NOT fall back to bare-button in production; it registers a leaf. The container shell has no ui:ai-panel zone ancestor (that zone is in the AiPanel body, which when collapsed is hidden but still registered with 0x0 rect). Wrapping rail in its own ui:ai-panel zone would DUPLICATE the (hidden) body zone -> registration ambiguity. So rail leaf FQM will be /window/ui:ai-panel.expand directly under the window layer. That is a real, navigable, jump-enumerable leaf.
    - Container test (ai-panel-container.test.tsx) mounts WITHOUT a FocusLayer -> covers graceful-degradation (rail renders as plain labelled button, sparkles icon) — existing test already asserts this at the rail.
    Plan: TDD - add spatial assertions for header collapse leaf+Enter to ai-panel.spatial.test.tsx (uses existing renderPanel/renderPanelWithShell). For rail, add a render helper mounting AiPanelContainer inside a FocusLayer to assert the rail leaf registers + Enter fires onToggle.
  timestamp: 2026-06-22T14:51:45.910293+00:00
- actor: claude-code
  id: 01kvqxfwqswdjwr3w3de6veyhr
  text: |-
    Implemented (TDD red->green confirmed).

    Files changed:
    - src/components/ai-panel.tsx: header collapse Button -> AiPanelPressable moniker "ui:ai-panel.collapse", onPress=onCollapse, ariaLabel preserved, ghost-icon style via buttonVariants({variant:"ghost",size:"icon"}). Imported buttonVariants.
    - src/components/ai-panel-container.tsx: rail expand Button -> AiPanelPressable moniker "ui:ai-panel.expand", onPress=onToggle, ariaLabel preserved, same buttonVariants className. Replaced Button import with buttonVariants + added AiPanelPressable + asSegment imports.
    - src/components/ai-panel.spatial.test.tsx: added header collapse leaf-registration, jump-to, and Enter->onCollapse assertions (using existing renderPanel/renderPanelWithShell, now accepting an onCollapse spy); added a NEW "collapsed rail expand control" describe that mounts the REAL AiPanelContainer collapsed inside the window FocusLayer (new renderCollapsedRail helper + window-container/command-scope/ai_list_models mocks) and asserts the rail leaf registers, is jump-to enumerable, and Enter toggles the panel open (data-ai-panel-collapsed false).

    Rail zone question: rail did NOT need a new zone. AiPanelContainer is already a child of App.tsx window FocusLayer, so AiPanelPressable registers a real leaf at /window/ui:ai-panel.expand (directly under the window layer; the container shell has no ui:ai-panel zone ancestor, and wrapping it in one would duplicate the hidden body's ui:ai-panel zone -> ambiguity). The new spatial test proves the leaf truly registers + is navigable + Enter-activatable.

    Evidence:
    - BEFORE fix: the 6 new assertions failed with "expected null to be truthy" (no FocusScope leaf for bare <Button>); collapsed-state precondition passed.
    - AFTER fix:
      - npx tsc --noEmit: clean (exit 0)
      - ai-panel.spatial.test.tsx: 20 passed (14 pre-existing + 6 new)
      - ai-panel.test.tsx (browser): 25 passed (header graceful-degradation/click intact)
      - ai-panel-container.test.tsx: rail graceful-degradation already covered (labelled clickable sparkles button outside FocusLayer + onToggle on click) - passes
      - ai-panel-elicitation.spatial.test.tsx + ai-panel-dock.spatial.test.tsx: 33 passed (no regression)
      - combined gate run: 62 passed (3 files)
    Note: ai-panel.test.tsx is a BROWSER project test (not unit/node), so run with --project browser; the task's --project unit invocation matched 0 files.
  timestamp: 2026-06-22T14:59:27.609427+00:00
- actor: claude-code
  id: 01kvqxnm804dbg87va4k34m0n2
  text: 'really-done adversarial double-check: PASS. Reviewer independently re-ran gates (tsc exit 0; 62 tests passed across ai-panel.spatial/ai-panel/ai-panel-container) and cleared all checks: new spatial tests exercise real production registration (non-vacuous), rail test mounts the REAL AiPanelContainer, ghost-icon styling equivalent via buttonVariants, aria-labels+mouse-click intact on both AiPanelPressable code paths, no FQM collision (/window/ui:ai-panel.expand vs /window/ui:ai-panel/ui:ai-panel.collapse vs zone /window/ui:ai-panel). Latent note (advisory, out of scope): the hidden collapsed body still registers its ui:ai-panel.collapse leaf in a display:none subtree — pre-existing behavior, no collision, not part of this fix. Moving to review.'
  timestamp: 2026-06-22T15:02:35.520380+00:00
- actor: claude-code
  id: 01kvqy6mdm9x59jx9fcd7typas
  text: Picked up the 3 test-scaffolding nits from Review Findings (2026-06-22 10:11). Production files untouched. Working the spatial test file only.
  timestamp: 2026-06-22T15:11:52.756936+00:00
- actor: claude-code
  id: 01kvqyaxc7yz2ebv115c0a79pc
  text: |-
    Addressed all 3 test-scaffolding nits in ai-panel.spatial.test.tsx (production files untouched):

    1. renderCollapsedRail now leads with a doc comment stating it renders the AI panel in collapsed-rail state to exercise the rail's expand affordance.
    2. Extracted module-level `const TEST_VIEWPORT_WIDTH = 1200;` / `const TEST_VIEWPORT_HEIGHT = 700;` and reused them across all three harness functions (renderPanel, renderPanelWithShell, renderCollapsedRail) via template literals. No `1200px`/`700px` literals remain.
    3. Extracted module-level `const SETUP_FLUSH_TIMEOUT_MS = 80;` and used it in flushSetup.

    Gates (actual):
    - `npx tsc --noEmit` -> TSC_EXIT=0
    - `npx vitest run --project browser src/components/ai-panel.spatial.test.tsx` -> Test Files 1 passed (1), Tests 20 passed (20), VITEST_EXIT=0
    - `npx prettier --write` on the file -> exit 0 (no reformatting needed beyond run)

    All 3 nit checkboxes flipped to [x]. Task remains in review.
  timestamp: 2026-06-22T15:14:12.999683+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffdb80
project: ui-command-cleanup
title: AI panel toggle (collapse/expand) buttons are not focusable or jump-to reachable
---
## What

The AI panel's collapse/expand toggle (the sparkles "AI star" button) cannot be reached by keyboard — neither via spatial navigation (arrow keys) nor the jump-to overlay (vim `s` / CUA `Mod+G`), and it has no Enter/Space activation. Every other interactive control in the panel participates in the app's spatial-nav graph; these two toggle buttons do not.

Root cause: both toggle buttons are rendered as a bare `<Button>` (from `@/components/ui/button`) instead of the panel's focus-scope primitive `AiPanelPressable`. A bare `<Button>` mounts no `<FocusScope>` leaf, so the spatial-nav kernel and the jump-to overlay never see it, and there is no scope-level `pressable.activate` CommandDef for Enter/Space.

There are **two** instances of the bug — the expanded panel header and the collapsed rail:

1. **Header collapse button** — `apps/kanban-app/ui/src/components/ai-panel.tsx:312-328` (`AiPanelHeader`), `aria-label="Collapse AI panel"`, wired to `onCollapse`. This lives inside the `ui:ai-panel` zone (the `AiPanel` body sets `moniker={asSegment("ui:ai-panel")}` at `ai-panel.tsx:253`).
2. **Rail expand button** — `apps/kanban-app/ui/src/components/ai-panel-container.tsx:679-690`, `aria-label="Expand AI panel"`, wired to `onToggle`, rendered only when `!open`.

### Fix

Replace each bare `<Button>` with `<AiPanelPressable>` (defined in `apps/kanban-app/ui/src/components/ai-panel-focus.tsx:141`), following the exact pattern already used by the message copy/retry buttons (`ai-panel.tsx:690-712`) and the elicitation action buttons (`ai-panel.tsx:1085+`):

```tsx
<AiPanelPressable
  moniker={asSegment("ui:ai-panel.collapse")}   // header; rail: "ui:ai-panel.expand" (or a shared "ui:ai-panel.toggle")
  onPress={onCollapse}                            // rail: onToggle
  ariaLabel="Collapse AI panel"                   // rail: "Expand AI panel"
>
  <SparklesIcon className="size-4" />
</AiPanelPressable>
```

`AiPanelPressable` mounts a `<Pressable>` (FocusScope leaf + Enter/Space CommandDefs) when inside a `<FocusLayer>`, and degrades to a plain labelled `<button>` outside one (so standalone unit tests still render). Keep `size`/`variant` styling equivalent — pass through `className` if `AiPanelPressable` does not forward `size`/`variant`; match the existing ghost-icon look.

**Verify the rail button has a focus-layer/zone ancestor.** The header button sits inside the `ui:ai-panel` zone, but the rail button in `ai-panel-container.tsx` is in the container shell, not the `AiPanel` body. If the collapsed rail is not under a `<FocusLayer>`/zone that the kernel registers, `AiPanelPressable` will silently fall back to the bare-button path and the rail control still won't be jumpable. If so, register the rail control under the appropriate zone (mirror how the header zone is established) so it appears in spatial nav and jump-to. The spatial test below is the gate that proves this.

## Acceptance Criteria
- [ ] The expanded-panel header collapse button is an `AiPanelPressable` with a stable moniker (e.g. `ui:ai-panel.collapse`) and is reachable by spatial navigation and the jump-to overlay.
- [ ] The collapsed rail expand button is an `AiPanelPressable` with a stable moniker (e.g. `ui:ai-panel.expand`) and is reachable by spatial navigation and the jump-to overlay.
- [ ] Pressing Enter (and Space under CUA keymap) on the focused toggle invokes `onCollapse` / `onToggle` — the panel folds/unfolds.
- [ ] Mouse click behavior and `aria-label`s are unchanged; the ghost-icon sparkles styling is visually preserved.
- [ ] No regression in the existing AI panel spatial and unit tests.

## Tests
- [ ] Extend `apps/kanban-app/ui/src/components/ai-panel.spatial.test.tsx` (the real-Chromium spatial harness using `@/test/spatial-shadow-registry`) with cases that:
  - assert the header collapse control registers a FocusScope leaf at FQM `/window/ui:ai-panel/ui:ai-panel.collapse` (mirror the existing model-selector assertion at `ai-panel.spatial.test.tsx:458`, `composeFq(zoneFq, asSegment("ui:ai-panel.collapse"))`);
  - assert the collapsed rail expand control registers its leaf and is enumerable by jump-to (follow the existing "jump-to landing on panel controls" pattern in this file);
  - drive Enter on the focused collapse scope and assert `onCollapse` fired; drive Enter on the rail expand scope and assert `onToggle` fired (mirror the elicitation keyboard-activation assertion referenced at `ai-panel-elicitation.spatial.test.tsx:1020`, which fails if a control reverts from `AiPanelPressable` to bare `<Button>`).
- [ ] Keep/extend the standalone unit coverage in `apps/kanban-app/ui/src/components/ai-panel.test.tsx` to confirm the toggle still renders a labelled, clickable button outside a `<FocusLayer>` (the `AiPanelPressable` graceful-degradation path).
- [ ] Run `cd apps/kanban-app/ui && npm test` (`tsc --noEmit && vitest run`) — both the `unit` (happy-dom) and `browser` (Chromium/Playwright) projects pass green.

## Workflow
- Use `/tdd` — add the failing spatial assertions for both toggle monikers + Enter activation first, watch them fail (they will, since the controls are bare `<Button>`s today), then swap to `AiPanelPressable` to make them pass. #bug

## Review Findings (2026-06-22 10:11)

Scope: uncommitted working diff (`ai-panel.tsx`, `ai-panel-container.tsx`, `ai-panel.spatial.test.tsx`). Engine container-file findings outside the changed hunks (pre-existing `n`/`err`/`e` naming, `AiPanelContainerBody`/`AiPanelShell` length, the `0.85` literal) were confirmed pre-existing and excluded. Core correctness verified clean: rail leaf registers under `/window` (`expandLeaf.layerFq === windowLayerFq`), header leaf at `/window/ui:ai-panel/ui:ai-panel.collapse` — distinct paths, no ambiguity; Enter activation fires `onCollapse` and flips `data-ai-panel-collapsed`; aria-labels + ghost-icon styling preserved; graceful-degradation unit path green. Spatial 20/20, unit 25/25, `tsc --noEmit` exit 0.

### Nits (new test scaffolding only)
- [x] `apps/kanban-app/ui/src/components/ai-panel.spatial.test.tsx` — helper `renderCollapsedRail` lacks a doc comment explaining it renders the AI panel in collapsed-rail state to exercise the expand affordance. Add a brief comment.
- [x] `apps/kanban-app/ui/src/components/ai-panel.spatial.test.tsx` — test-container dimensions `1200` / `700` are hardcoded across harness functions. Extract module-level constants (e.g. `TEST_VIEWPORT_WIDTH`, `TEST_VIEWPORT_HEIGHT`) and reuse.
- [x] `apps/kanban-app/ui/src/components/ai-panel.spatial.test.tsx` — the setup-flush timeout `80` (ms) is a magic number. Extract `const SETUP_FLUSH_TIMEOUT_MS = 80;` at module level.