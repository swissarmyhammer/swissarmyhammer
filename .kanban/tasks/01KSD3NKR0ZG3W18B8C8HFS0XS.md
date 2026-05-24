---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffa980
title: Chat copy must populate the vim register so CM6 `p` pastes it
---
## What

In the AI chat, the per-message **Copy** button (and the code-block copy buttons inside tool folds) only writes the OS clipboard via `navigator.clipboard.writeText`. So after copying, pressing **`p`** in a CM6 vim editor (the composer, or any field editor) pastes nothing — the copied text never lands where vim's `p` reads.

### Root cause (researched)

- `MessageActionBar.handleCopy` in `apps/kanban-app/ui/src/components/ai-panel.tsx` did `navigator.clipboard.writeText(text)`. The code-block copy in `apps/kanban-app/ui/src/components/ai-elements/code-block.tsx` did the same.
- The app's vim mode is `@replit/codemirror-vim` (`apps/kanban-app/ui/src/lib/cm-keymap.ts` → `vim()`).
- In `@replit/codemirror-vim`, plain `p` pastes **synchronously** from the **unnamed register** (`"`); only `"+p` reads the OS clipboard. The unnamed register is populated by in-editor yanks/deletes, never by an external `writeText`. Hence bare `p` saw an empty/stale register.
- The vim register controller is module-global and exposed via `Vim.getRegisterController()`.

### Approach (done)

1. New `apps/kanban-app/ui/src/lib/clipboard.ts` — `copyText(text)` mirrors the text into the vim registers (unnamed `"`, `0`, `+`) via `Vim.getRegisterController()` (best-effort, try/catch) **and** `await navigator.clipboard.writeText(text)`.
2. Routed both copy sites through `copyText`: `ai-panel.tsx` `MessageActionBar.handleCopy` and the vendored `ai-elements/code-block.tsx` (minimal swap, no reformat).

### Non-goals

- No vim keybinding changes; no attempt to make bare `p` async-read the OS clipboard (codemirror-vim's non-`+` paste is synchronous).
- CUA/emacs paste unaffected.

## Acceptance Criteria

- [x] After `copyText(text)` runs, a CM6 editor in vim mode pastes `text` on a bare `p` (normal mode) — not just on `"+p`.
- [x] `copyText` still writes the OS clipboard (CUA/emacs paste and system paste unaffected).
- [x] Both the per-message Copy button and the tool-fold code-block Copy button route through `copyText`.
- [x] When the vim register controller is unavailable, `copyText` still writes the OS clipboard and does not throw.

## Tests

- [x] Unit test `apps/kanban-app/ui/src/lib/clipboard.test.ts`: `copyText("hi")` calls `writeText("hi")` and sets the unnamed (`"`) and `0` registers; tolerates `getRegisterController` throwing.
- [x] Behavioral browser test in `ai-prompt-composer.test.tsx`: vim-mode CM6 editor, `copyText("PASTE_ME")`, real bare `p`, asserts the buffer contains `PASTE_ME`. Fail-before / pass-after.
- [x] `pnpm --filter ./apps/kanban-app/ui test clipboard` and `... test ai-prompt-composer` green.
- [x] Full UI suite green: 257 files, 2443 tests; `tsc --noEmit` clean.

### Implementation note

The behavioral browser test runs in headless Chromium, which denies `clipboard-write`, so the real `writeText` is pinned by the unit test (spy) while the behavioral test drives the vim-register mirror with a real bare-`p` keystroke. It fails before the fix (no mirror → `p` pastes nothing) and passes after.

## Review Findings (2026-05-24 17:30 — task-mode, reviewer subagent)

0 blockers, 1 warning, 1 nit — both resolved.

### Warnings
- [x] `clipboard.ts` `copyText` ordering — the mirror was gated on `writeText` success (`await writeText` first), so a denied OS write (restricted clipboard permission) would skip the vim mirror and leave bare `p` empty — gating the robust feature on the fragile one. Fixed: `copyText` now mirrors to the vim registers FIRST, then `await navigator.clipboard.writeText(text)` (write error still re-thrown for caller logging). So `p` works even when the OS write is refused.

### Nits
- [x] `clipboard.test.ts` `makeRegister` — abbreviated `setText(t?)` param renamed to `text` per the JS/TS no-abbreviations guideline.

Re-verified after fixes: full UI suite green (257 files, 2443 tests; tsc clean).

#bug