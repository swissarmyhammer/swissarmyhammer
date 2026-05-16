---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
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
- [ ] AI Elements components are vendored under `ui/src/components/ai-elements/` and type-check against the project's React 19 / Tailwind 4 setup.
- [ ] `npm run build` (tsc + vite) in `apps/kanban-app/ui` succeeds.

## Tests
- [ ] Vitest smoke test: render each vendored component with representative sample props and assert it mounts without error.
- [ ] `npm test` (`tsc --noEmit && vitest run`) in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — add the smoke render tests as each component is vendored.