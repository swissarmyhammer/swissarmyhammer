---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffef80
project: ai-panel
title: Vendor AI Elements components into the UI
---
## What
Bring the AI Elements component library into the webview. AI Elements is shadcn-style — components are copied into the project (the repo already uses shadcn: `components.json`, Radix, Tailwind 4).

- Install AI Elements components into `apps/kanban-app/ui/src/components/ai-elements/` via the AI Elements CLI — at minimum `Conversation`, `Message`, `Response`, `Reasoning`, `Tool`, `Task`, `PromptInput`, `Loader`, `Actions`.
- Add any npm dependencies the components require to `apps/kanban-app/ui/package.json`.
- These components are presentational — they will be rendered directly from the ACP conversation state (a later task), NOT driven by the AI SDK `useChat` hook / a `ChatTransport`.

Reference: https://elements.ai-sdk.dev/docs

## Acceptance Criteria
- [x] AI Elements components are vendored under `ui/src/components/ai-elements/` and type-check against the project's React 19 / Tailwind 4 setup.
- [x] `npm run build` (tsc + vite) in `apps/kanban-app/ui` succeeds.

## Tests
- [x] Vitest smoke test: render each vendored component with representative sample props and assert it mounts without error.
- [x] `npm test` (`tsc --noEmit && vitest run`) in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — add the smoke render tests as each component is vendored.

## Implementation Notes
Vendored via the shadcn CLI against `registry.ai-sdk.dev` (the `ai-elements` CLI's `elements.ai-sdk.dev/api/registry` endpoint 404s on `loader`/`response`/`actions`; the two registries are out of sync). 9 files under `src/components/ai-elements/`: `conversation`, `message`, `reasoning`, `tool`, `task`, `prompt-input`, `loader`, plus registry deps `code-block` and `shimmer`.

In the current AI Elements registry, `Response` and `Actions` no longer have standalone registry items — they were consolidated into the `message` component as `MessageResponse` (markdown response) and `MessageActions`/`MessageAction` (action buttons). Vendoring `message` satisfies the "Response" and "Actions" minimum.

The `ai` package is used only for `import type` (`UIMessage`, `ToolUIPart`, `ChatStatus`, `FileUIPart`) — no `useChat`/`ChatTransport` runtime; components stay presentational.

npm deps added to `package.json`: `ai`, `streamdown`, `use-stick-to-bottom`, `@radix-ui/react-use-controllable-state`, `nanoid`, `shiki`, `cmdk`, `motion`. New shadcn UI dependency components added under `ui/`: `collapsible`, `button-group`, `command`, `dialog`, `hover-card`, `input`, `input-group`, `textarea`. Pre-existing `ui/` components the CLI tried to restyle (semicolon-only diffs) were reverted to keep project Prettier style.

Three vendored-code fixes for the project's stricter tsconfig (ES2021 / React 19 / `noUnused*`): `prompt-input.tsx` `.at(-1)` → index access; `reasoning.tsx` stop spreading collapsible props onto `Streamdown`; `tool.tsx` removed two now-unused `@ts-expect-error` directives (the installed `ai@6` includes the `approval-requested` state).

Smoke tests: `src/components/ai-elements/ai-elements.smoke.test.tsx` — 12 tests covering all 9 files, 12/12 pass. `npm run build` is green. `npm test` for the AI-Elements scope is green; the full suite has 3 pre-existing failures (`slugify.parity.node.test.ts`, `editor-save.test.tsx`) caused by stale `apps/swissarmyhammer-*` fixture paths after the `apps/` crate move — unrelated to this task, reproduce on unmodified HEAD, tracked as `01KRS426Q36ZN3DYBX2S0AS82T`.