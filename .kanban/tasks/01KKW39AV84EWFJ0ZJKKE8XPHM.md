---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffbd80
title: refresh.test.ts uses `any` type — violates no-any guideline
---
kanban-app/ui/src/lib/refresh.test.ts:4,8\n\n```ts\nconst mockInvoke = vi.fn((..._args: any[]) => Promise.resolve({}));\n// ...\nvi.mock(\"@tauri-apps/api/core\", () => ({\n  invoke: (...args: any[]) => mockInvoke(...args),\n}));\n```\n\nBoth usages carry an `// eslint-disable-next-line @typescript-eslint/no-explicit-any` comment, signalling awareness, but the JS/TS guidelines require `unknown` over `any`. In test files this is low risk, but it sets a bad precedent.\n\nSuggestion: Change the mock signatures to use `unknown[]`:\n```ts\nconst mockInvoke = vi.fn((..._args: unknown[]) => Promise.resolve({}));\n```"