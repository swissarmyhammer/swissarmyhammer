# Python Test Coverage Conventions

## Test File Locations

- **Mirror layout:** `tests/` directory mirroring the source tree. `src/mypackage/parser.py` → `tests/test_parser.py` or `tests/mypackage/test_parser.py`.
- **`test_` prefix:** Test files are named `test_<module>.py`. Test functions are named `test_<behavior>`.
- **`conftest.py`:** Shared fixtures, not tests. Do not analyze for coverage.

For a source file `src/mypackage/config.py`, look for:
1. `tests/test_config.py`
2. `tests/mypackage/test_config.py`
3. `tests/unit/test_config.py`

## Treesitter AST Queries

**Find functions and methods:**
```scheme
(function_definition
  name: (identifier) @name)

(class_definition
  name: (identifier) @class_name
  body: (block
    (function_definition
      name: (identifier) @method_name)))
```

**Find test functions:**
```scheme
(function_definition
  name: (identifier) @name
  (#match? @name "^test_"))
```

**Find class definitions:**
```scheme
(class_definition
  name: (identifier) @name)
```

## What Requires Tests

- All public functions (no leading underscore)
- All public methods on classes
- Class `__init__` with validation or transformation logic
- Functions decorated with `@app.route`, `@router.get`, etc. (endpoint handlers)
- Error handling branches (`except` clauses with business logic)
- Factory functions and builder patterns
- Data validation logic (Pydantic validators, attrs validators)

## Acceptable Without Direct Tests

- Private functions (`_helper()`) called exclusively from tested public functions
- `__repr__`, `__str__` unless they contain business logic
- Simple dataclass/attrs definitions with no custom methods
- Type aliases and constants
- `if __name__ == "__main__"` blocks

## Test Naming Conventions

Python tests follow: `test_<function_name>`, `test_<function_name>_<scenario>`, or `test_<class_name>_<method_name>`. Match source function names against test function names with the `test_` prefix.

## Testing Patterns

- `pytest` with `assert` statements (not `unittest.TestCase`)
- `@pytest.fixture` for setup/teardown
- `@pytest.mark.parametrize` for parameterized tests
- `pytest.raises(ExceptionType)` for error-path tests
- `unittest.mock.patch` or `pytest-mock` for mocking (prefer owned facades)
- `hypothesis` for property-based testing
