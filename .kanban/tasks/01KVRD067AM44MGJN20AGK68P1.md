---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvzjd7hmw7ztswxxdrfdarcf
  text: |-
    Picked up. TDD red-first: added "truncates a long model label..." test under the footer-select describe block — renders the composer in a 320px wrapper with a pathologically long model label, asserts toolbar.scrollWidth <= clientWidth and value scrollWidth > clientWidth.

    KEY DISCOVERY (the card's premise was off): the browser test project did NOT apply Tailwind. vite.config.ts gave the browser vitest project plugins:[react()] only (no tailwindcss()), and no test imported the app CSS — so flex/min-w-0/line-clamp-1 produced ZERO CSS rules in the Chromium DOM (verified: 0 matching rules; toolbar computed display was "block", trigger "inline-block"). Layout assertions were meaningless. Root-caused via computed-style probes.

    Fix for the test infra (minimal, additive): added tailwindcss() to the browser vitest project's plugins and `import "@/index.css"` at the top of the test file so the real utility CSS lands in the test DOM. Existing tests use inline styles / class-presence, so adding real Tailwind is additive, not breaking.

    Production fix (call-site only, exactly as specced): `className="min-w-0"` on the ComposerModelSelect PromptInputSelectTrigger + `shrink-0` on the footer submit/stop button. Confirmed min-w-0 alone is sufficient (value clamps via the base trigger's existing line-clamp-1 once the trigger can shrink) — did NOT touch shared SelectTrigger/SelectValue/PromptInputSelectTrigger.

    RED evidence (Tailwind applied, fix removed): toolbar.scrollWidth 861 > clientWidth 318 (overflow). GREEN (fix in): toolbar 318<=318, value scrollWidth 556 > clientWidth 218 (clamped), trigger fits at 266. Next: full ai-prompt-composer file + tsc.
  timestamp: 2026-06-25T14:19:41.492291+00:00
- actor: claude-code
  id: 01kvzk2y51veb9r5kmrnkqvwh1
  text: |-
    Correction to my earlier verification note (per adversarial double-check, which re-ran baseline vs with-fix): the full UI browser suite's pre-existing-broken baseline is larger than I first cited. It is 10 FAILED tests + 7 files that error at import/suite level (avatar, board-selector, attachment-editor, comment-log-editor, single-select-editor, avatar-display, comment-log-display — all from a stale @tauri-apps/api vite dep-prebundle cache: "does not provide an export named SERIALIZE_TO_IPC_FN"). The 10 failing tests: editor-save x6, mention-view x1, entity-card (entity.inspect moniker), grid-empty-state (context-menu view scope), spatial-nav-end-to-end (perspective.switch).

    CRITICAL: these are NOT regressions. Confirmed by running the FULL browser suite both ways: baseline (my 3 files git-stashed → browser project back to plugins:[react()], no Tailwind) = 10 failed / 2431 passed; with-fix = 10 failed / 2432 passed. Byte-identical failure set; the only delta is my +1 new truncation test passing. The tailwindcss() plugin addition is regression-free.

    Why other browser test files are unaffected by the plugin: Tailwind v4 emits utility CSS only at the `@import "tailwindcss"` site, and `@/index.css` is imported ONLY in ai-prompt-composer.test.tsx — so the project-wide plugin is a no-op for every other browser test file (empirically confirmed by identical baseline/with-fix failure sets).

    Double-check verdict: code is correct, on-intent, call-site-only (shared SelectTrigger/SelectValue/PromptInputSelectTrigger/Pressable untouched), non-tautological, tsc clean. min-w-0 reaches the real combobox button (className flows trigger→SelectTrigger→SelectPrimitive.Trigger via cn(); Slot.Root merges, doesn't drop). The only REVISE item was this evidence-accuracy correction — now logged. Work is done and green; leaving in doing for /review.
  timestamp: 2026-06-25T14:31:32.769989+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffed80
project: ai-panel
title: AI panel model selector overflows on long model names instead of truncating with ellipsis
---
## What

**Bug**: In the AI panel composer footer, a long model label (e.g. the Claude Code CLI model name) makes the model-selector trigger grow to its full content width and overflow the panel horizontally, instead of truncating with an ellipsis (`…`).

**Root cause** — the footer model picker is `ComposerModelSelect` in `apps/kanban-app/ui/src/components/ai-prompt-composer.tsx` (around line 311), which renders a `PromptInputSelectTrigger` (→ shadcn `SelectTrigger` in `apps/kanban-app/ui/src/components/ui/select.tsx:25`). The base `SelectTrigger` className (select.tsx:38) is `flex w-fit … whitespace-nowrap` and applies `line-clamp-1` to the value slot (`*:data-[slot=select-value]:line-clamp-1`). But:
- `w-fit` sizes the trigger to its content, and there is no `max-w` or `min-w-0`.
- As a flex item in the footer toolbar (`<div className="flex items-center justify-between gap-2 …">`, ai-prompt-composer.tsx:664), the trigger's default `min-width: auto` is its content width, so the flex item never shrinks below the full label width.

With nothing allowing the trigger to shrink, `line-clamp-1` never has a constrained width to clamp against, so the label renders full-width and pushes the footer past the panel edge.

### Fix approach (call-site only — do NOT touch shared `SelectTrigger`/`PromptInputSelectTrigger`, used elsewhere)

In `apps/kanban-app/ui/src/components/ai-prompt-composer.tsx`:
- [ ] Add `min-w-0` to the `PromptInputSelectTrigger` in `ComposerModelSelect` (pass via its `className` — `PromptInputSelectTrigger` forwards `className` through `cn(...)`). This lets the flex item shrink below content width so the existing `line-clamp-1` on the value slot clamps and shows the ellipsis. Optionally also cap with a `max-w` if needed for the narrow dock.
- [ ] Add `shrink-0` to the footer submit/stop `<button>` (ai-prompt-composer.tsx:681 className block) so the fixed `size-7` action button is never compressed when the selector shrinks.
- [ ] If the `AiPanelPressable asChild` wrapper interferes with class merging onto the trigger, apply `min-w-0` directly on the element that becomes the `role="combobox"` button.

### Notes
- The UI test project runs in **real Chromium via Playwright** with Tailwind applied (`apps/kanban-app/ui/vite.config.ts`), so a real layout/overflow assertion is possible — no class-only proxy needed.

## Acceptance Criteria
- [ ] With a pathologically long model label and the composer constrained to a narrow width (e.g. 320px, the dock width), the footer toolbar does not overflow its container: the toolbar's `scrollWidth` is `<=` the container's `clientWidth`.
- [ ] The model-selector value is clamped (the value element's `scrollWidth > clientWidth`), i.e. the label is truncated rather than shown in full.
- [ ] The submit/stop action button keeps its `size-7` footprint (not compressed) when the label is long.
- [ ] Short model labels still render fully (no regression to existing footer-select tests).

## Tests
- [ ] In `apps/kanban-app/ui/src/components/ai-prompt-composer.test.tsx`, add a test under the existing `describe("AiPromptComposer — footer model select", …)` block: render `<AiPromptComposer …>` inside a `<div style={{ width: 320 }}>` with a model whose `label` is a long string (e.g. `"claude-opus-4-8[1m] Anthropic Claude Code CLI — very long model display name"`) and `selectedModel` set to it. Query the footer toolbar element and assert `toolbar.scrollWidth <= toolbar.clientWidth` (no horizontal overflow), and query the `role="combobox"` trigger's value element (`[data-slot='select-value']`) and assert `valueEl.scrollWidth > valueEl.clientWidth` (clamped). This fails before the fix (trigger overflows) and passes after.
- [ ] Keep/verify the existing footer-select tests pass: `screen.getByRole("combobox", { name: /claude code/i })` still resolves for the standard `MODELS` fixture.
- [ ] Run: `cd apps/kanban-app/ui && npm test -- ai-prompt-composer` — all pass.
- [ ] Run the full UI suite: `cd apps/kanban-app/ui && npm test` — no regressions.

## Workflow
- Use `/tdd` — write the narrow-width overflow test first (it fails: the toolbar overflows / the value is not clamped), then add `min-w-0` to the trigger and `shrink-0` to the action button so it passes. #ui

## Review Findings (2026-06-25 08:46)

Scope: working tree vs HEAD (`62d30cb1a`), three touched files — `ai-prompt-composer.tsx`, `ai-prompt-composer.test.tsx`, `vite.config.ts`. FRONTEND-only (tsc + vitest, no cargo).

### In-scope verdict: CLEAN — no blockers, no in-scope warnings

Independently verified:
- [x] **tsc clean** — `npx tsc --noEmit` exit 0.
- [x] **Targeted tests pass** — `vitest run ai-prompt-composer.test.tsx` = 20/20 passed, including the new RED-first truncation test and the existing `getByRole("combobox", { name: /claude code/i })` footer-select tests (short labels unregressed).
- [x] **Call-site-only confirmed** — zero diff to shared `ui/select.tsx` and `prompt-input*`. `min-w-0` is applied only on the `ComposerModelSelect` `PromptInputSelectTrigger` (flows trigger→SelectTrigger→SelectPrimitive.Trigger via `cn()`); `shrink-0` only on the footer submit/stop button.
- [x] **vite.config.ts Tailwind change is regression-free (the high-risk item)** — adding `tailwindcss()` to the browser vitest project emits utility CSS only at the `@import "tailwindcss"` site, which lives in `index.css`. `@/index.css` is imported by exactly ONE test file (`ai-prompt-composer.test.tsx:23`); no global vitest browser setup loads CSS (`src/test/setup.ts` has no CSS import). Therefore the plugin is a no-op for every other browser test — structurally corroborating the implementer's empirical A/B (10 failed / 2431 passed baseline → 10 failed / 2432 passed with fix, only +1 new test). Spot-checked layout-sensitive browser tests (`card-column-fit`, `ai-panel-dock.spatial`) pass cleanly with the plugin in place. No existing geometry test depends on Tailwind being absent (none import `index.css`).
- [x] **Genuine real-layout test** — new test asserts real `scrollWidth`/`clientWidth` (RED 861>318 overflow → GREEN 318<=318 + value clamped), not a class-name proxy.

### Mechanism note (not a blocker — judgment for future work)
- [ ] The CSS is loaded via `import "@/index.css"` inside the single test file rather than globally in the vitest browser setup (`src/test/setup.ts`). This is the correct minimal choice for THIS task (keeps the blast radius to one file and the suite green), but if more browser tests later need real Tailwind for layout assertions, consider moving the `@/index.css` import into the browser-project `setupFiles` so all layout tests share consistent CSS. Out of scope here; flagging for consistency.

### Pre-existing whole-file noise (NOT this task's diff — disregard for verdict)
The review engine surfaced several findings on untouched code; none are introduced by this diff:
- [ ] `ai-prompt-composer.tsx:185,304` — `selectedModel: AiModel | null` should be `| undefined` per `no-null` rule. Pre-existing field type, untouched by this change.
- [ ] `ai-prompt-composer.tsx:501` — `AiPromptComposer` exceeds the 50-line function threshold. Pre-existing component shape.
- [ ] `ai-prompt-composer.test.tsx:139,147,179,273,288,307` — duplicated `timeout: 2000` literals should be a named constant. Pre-existing autocomplete tests, untouched.
- [ ] `vite.config.ts:5` — `import path from "path"` should use `node:path`. Pre-existing import.
- [ ] `ai-prompt-composer.test.tsx:411` — the `320` dock-width literal is the only in-diff line flagged; it is intentional dock-width simulation and well-commented. Nit only.

Pre-existing browser-suite failures (editor-save ×6, mention-view, entity-card, grid-empty-state, spatial-nav-end-to-end + stale `@tauri-apps/api` dep-prebundle import errors) are NOT regressions from this task — confirmed identical baseline/with-fix failure set.