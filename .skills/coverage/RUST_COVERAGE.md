# Rust Test Coverage Conventions

## Test File Locations

- **Inline tests:** `#[cfg(test)] mod tests { ... }` at the bottom of the source file. Most common for unit tests.
- **Integration tests:** `tests/` directory at the crate root. Each `.rs` file is a separate test binary.
- **Doc tests:** `///` comments with code blocks on public items.

For a source file `src/parser.rs`, look for:
1. `#[cfg(test)]` module within `src/parser.rs` itself
2. `tests/parser.rs` or `tests/parser/*.rs` in the crate root

## Treesitter AST Queries

**Find public functions and methods:**
```scheme
(function_item
  (visibility_modifier) @vis
  name: (identifier) @name)

(function_item
  name: (identifier) @name
  body: (block))
```

**Find test functions:**
```scheme
(attribute_item
  (attribute (identifier) @attr)
  (#eq? @attr "test"))
```

Test functions are annotated with `#[test]` or `#[tokio::test]`. Search for functions preceded by these attributes.

**Find impl blocks and methods:**
```scheme
(impl_item
  type: (_) @type
  body: (declaration_list
    (function_item
      name: (identifier) @method)))
```

**Find trait definitions:**
```scheme
(trait_item
  name: (type_identifier) @name)
```

## What Requires Tests

- All `pub` and `pub(crate)` functions
- All `impl` methods on public types
- Trait implementations (especially `From`, `TryFrom`, custom traits)
- Error variants and their `Display` implementations
- `unsafe` blocks — every unsafe operation needs a test proving safety invariants hold
- Match arms that handle error cases

## Acceptable Without Direct Tests

- Private helper functions called exclusively from tested public functions
- Derived trait implementations (`#[derive(Debug, Clone, ...)]`)
- Constants and type aliases
- Re-exports
- Build scripts (`build.rs`) unless they contain complex logic

## Test Naming Conventions

Rust tests typically follow: `test_<function_name>`, `test_<function_name>_<scenario>`, or `<function_name>_returns_error_on_invalid_input`. Match function names from the source file against test function names.

## Testing Patterns

- `#[should_panic]` for panic-path tests
- `assert_eq!`, `assert_ne!`, `assert!(matches!(...))` for value assertions
- `proptest` or `quickcheck` for property-based testing
- `tokio::test` for async tests
- `rstest` for parameterized tests with fixtures
