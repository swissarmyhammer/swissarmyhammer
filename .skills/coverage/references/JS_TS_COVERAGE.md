# JavaScript/TypeScript Test Coverage

## Running Coverage

**Vitest (preferred)**

```bash
# Full project
npx vitest run --coverage --reporter=lcov

# Specific directory or file
npx vitest run --coverage --reporter=lcov src/utils/
npx vitest run --coverage --reporter=lcov src/parser.ts
```

Output: `coverage/lcov.info`

If `@vitest/coverage-v8` or `@vitest/coverage-istanbul` is not installed:
```bash
npm install -D @vitest/coverage-v8
```

**Jest**

```bash
# Full project
npx jest --coverage --coverageReporters=lcov

# Specific directory or file
npx jest --coverage --coverageReporters=lcov src/utils/
```

Output: `coverage/lcov.info`

**pnpm workspaces**

```bash
# Specific package
pnpm --filter <package_name> run test -- --coverage --reporter=lcov
```

## Output

Both tools write `coverage/lcov.info`. Parse `DA:<line>,<hits>` lines per file.

## Scoping

- Pass specific files or directories as positional args to scope coverage
- In monorepos, use workspace filters (`pnpm --filter`, `nx run`)

## Test File Locations

- **Co-located:** `src/parser.ts` → `src/parser.test.ts` or `src/__tests__/parser.test.ts`
- **Separate directory:** `src/parser.ts` → `test/parser.test.ts` or `tests/parser.test.ts`
- **Spec naming:** `.spec.ts` is equivalent to `.test.ts`

## What Requires Tests

- All exported functions and classes
- All public class methods
- React/Vue/Svelte components with logic (conditionals, event handlers, effects)
- API route handlers and middleware
- Utility functions used across modules
- Custom hooks (`use*` functions)

## Acceptable Without Direct Tests

- Re-exports and barrel files (`index.ts` that only re-exports)
- Type definitions, interfaces, and type aliases
- Constants and configuration objects
- Private functions called exclusively from tested exports
- Generated code (GraphQL codegen, Prisma client)
