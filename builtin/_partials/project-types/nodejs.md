---
title: Node.js Project Guidelines
description: Best practices and tooling for Node.js projects
partial: true
---

### Node.js Project Guidelines

**Package Manager Detection:**
- Check for `package-lock.json` → use `npm`
- Check for `yarn.lock` → use `yarn`
- Check for `pnpm-lock.yaml` → use `pnpm`

**Common Commands:**
- Install dependencies: `npm install` / `yarn install` / `pnpm install`
- **Run ALL tests:** `npm test` / `yarn test` / `pnpm test`
- **Run specific test file:** `npm test -- path/to/test.js` / `yarn test path/to/test.js`
- Build: `npm run build` / `yarn build` / `pnpm build`
- Start dev server: `npm run dev` / `yarn dev` / `pnpm dev`
- Lint: `npm run lint` / `yarn lint` / `pnpm lint`

**IMPORTANT:** Do NOT glob for test files. Use `npm test` to run all tests - the test runner will discover them automatically.

**Best Practices:**
- Always run the appropriate package manager (don't mix npm/yarn/pnpm)
- Check `package.json` scripts section for available commands
- Use `npm ci` for clean installs in CI environments
- Run tests before committing changes

**File Locations:**
- Source code: `src/` or `lib/`
- Tests: `test/`, `tests/`, or `__tests__/`
- Configuration: Root directory (`.eslintrc`, `tsconfig.json`, etc.)
- Build output: `dist/`, `build/`, or `.next/` (git-ignored)
- Dependencies: `node_modules/` (git-ignored)
