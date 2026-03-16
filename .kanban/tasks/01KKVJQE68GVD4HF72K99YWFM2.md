---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffbc80
title: 'NIT: refresh.test.ts uses any[] type annotations with eslint-disable comments'
---
File: kanban-app/ui/src/lib/refresh.test.ts lines 3-8 — The mock uses `any[]` parameter types accompanied by two eslint-disable-next-line comments. Per the JS/TS review guidelines, `unknown` must be used over `any`; `any` requires specific documented justification.\n\nThe mock function signature can be typed precisely: the first argument is always a string (the command name), and the return is `Promise<unknown>`. Using `unknown` eliminates the need for the disable comments.\n\nSuggestion:\n```ts\nconst mockInvoke = vi.fn((..._args: unknown[]) => Promise.resolve({} as unknown));\nvi.mock(\"@tauri-apps/api/core\", () => ({ invoke: (...args: unknown[]) => mockInvoke(...args) }));\n```\n\nVerification step: remove the eslint-disable comments, change `any[]` to `unknown[]`, confirm TypeScript still compiles and tests still pass." #review-finding