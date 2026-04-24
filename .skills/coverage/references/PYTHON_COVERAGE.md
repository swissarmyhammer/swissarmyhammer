# Python Test Coverage

## Running Coverage

**pytest-cov (preferred)**

```bash
# Full project
pytest --cov=src --cov-report=lcov:lcov.info

# Specific package
pytest --cov=mypackage --cov-report=lcov:lcov.info tests/mypackage/

# Specific file
pytest --cov=src/mypackage/parser.py --cov-report=lcov:lcov.info tests/test_parser.py
```

Install if missing: `pip install pytest-cov`

**coverage.py directly**

```bash
coverage run -m pytest
coverage lcov -o lcov.info
```

Install if missing: `pip install coverage`

## Output

Both approaches write `lcov.info`. Parse `DA:<line>,<hits>` lines per file.

## Scoping

- `--cov=<path>` controls which source files are measured
- Pass specific test files or directories as positional args to scope the test run
- For monorepos, `cd` into the package directory first

## Test File Locations

- **Mirror layout:** `tests/test_<module>.py` or `tests/<package>/test_<module>.py`
- **`conftest.py`:** Shared fixtures, not tests

## What Requires Tests

- All public functions (no leading underscore)
- All public methods on classes
- Class `__init__` with validation or transformation logic
- Endpoint handlers (`@app.route`, `@router.get`)
- Error handling branches with business logic
- Data validation logic (Pydantic validators, attrs validators)

## Acceptable Without Direct Tests

- Private functions (`_helper()`) called exclusively from tested public functions
- `__repr__`, `__str__` unless they contain business logic
- Simple dataclass definitions with no custom methods
- Type aliases and constants
- `if __name__ == "__main__"` blocks
