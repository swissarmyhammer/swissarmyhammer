---
position_column: todo
position_ordinal: fc80
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