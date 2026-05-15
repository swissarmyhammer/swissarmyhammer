---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffb080
title: Migrate unit tests from jsdom to vitest browser mode
---
## What

The jsdom environment is broken for all DOM-based unit tests due to an ESM/CJS interop conflict: `cssstyle` (used by jsdom) tries to `require()` the `@asamuzakjp/css-color` package, which is ESM-only with top-level `await`. This causes `ERR_REQUIRE_ASYNC_MODULE` and prevents 73 test files from running. Only 2 pure-logic tests (`board-data-patch.test.ts`, `upsert-entity.test.ts`) pass.

The project already has a vitest browser mode config (`browser` project in `vite.config.ts`) using Playwright/Chromium. The fix is to migrate the `unit` test project from `environment: "jsdom"` to `browser` mode, eliminating the jsdom dependency entirely.

### Steps

- [ ] Remove the `unit` project from `vite.config.ts` test config (or change its environment from jsdom to browser)
- [ ] Rename `*.test.{ts,tsx}` files to `*.browser.test.{ts,tsx}` OR update the `browser` project include pattern to cover all tests
- [ ] Remove jsdom from devDependencies if no longer needed
- [ ] Verify all 75 test files run and pass in browser mode
- [ ] Fix any tests that rely on jsdom-specific APIs (e.g. `document.createElement` quirks)

## Why

jsdom is fundamentally broken with current dependencies, and the project intent is to use real browser testing via vitest browser mode anyway. This unblocks the entire test suite.