# JavaScript/TypeScript Test Coverage Conventions

## Test File Locations

- **Co-located:** `src/parser.ts` → `src/parser.test.ts` or `src/__tests__/parser.test.ts`
- **Separate directory:** `src/parser.ts` → `test/parser.test.ts` or `tests/parser.test.ts`
- **Spec naming:** `.spec.ts` is equivalent to `.test.ts`

For a source file `src/utils/config.ts`, look for:
1. `src/utils/config.test.ts` or `src/utils/config.spec.ts`
2. `src/utils/__tests__/config.test.ts`
3. `test/utils/config.test.ts` or `tests/utils/config.test.ts`

Also check `.js`, `.jsx`, `.tsx` variants of all the above.

## Treesitter AST Queries

**Find exported functions:**
```scheme
(export_statement
  declaration: (function_declaration
    name: (identifier) @name))

(export_statement
  declaration: (lexical_declaration
    (variable_declarator
      name: (identifier) @name)))
```

**Find functions and arrow functions:**
```scheme
(function_declaration
  name: (identifier) @name)

(variable_declarator
  name: (identifier) @name
  value: (arrow_function))
```

**Find class methods:**
```scheme
(class_declaration
  name: (identifier) @class_name
  body: (class_body
    (method_definition
      name: (property_identifier) @method_name)))
```

**Find test functions (Vitest/Jest):**
```scheme
(call_expression
  function: (identifier) @fn
  (#match? @fn "^(test|it|describe)$")
  arguments: (arguments
    (string) @test_name))
```

## What Requires Tests

- All exported functions and classes
- All public class methods
- React/Vue/Svelte components with logic (conditionals, event handlers, effects)
- API route handlers and middleware
- Utility functions used across modules
- Error handling and edge cases in exported functions
- Custom hooks (`use*` functions)

## Acceptable Without Direct Tests

- Re-exports and barrel files (`index.ts` that only re-exports)
- Type definitions, interfaces, and type aliases (test with `tsd` if complex)
- Constants and configuration objects
- Private functions called exclusively from tested exports
- Generated code (GraphQL codegen, Prisma client)

## Test Naming Conventions

JS/TS tests use `describe`/`it`/`test` blocks. Match source function names against strings in test descriptions: `describe('parseConfig', ...)` or `test('parseConfig returns default on empty input', ...)`.

## Testing Patterns

- **Vitest** (preferred) or **Jest** as test runner
- `describe` for grouping, `it`/`test` for individual cases
- `expect(value).toBe()`, `.toEqual()`, `.toThrow()` assertions
- `vi.fn()` / `jest.fn()` for mocks and spies
- `vi.mock()` / `jest.mock()` for module mocks
- `@testing-library/react` for component tests
- `msw` for API mocking
- `tsd` for type-level tests
