there are many tests that use a plain TempDir instead of the mandated IsolatedTestEnvironment. Fix them
## Proposed Solution

Based on my analysis, I found 262 instances of TempDir usage across 36 test files that need to be replaced with IsolatedTestEnvironment. The main patterns I need to fix are:

1. **Import statements**: Replace `use tempfile::TempDir;` with `use swissarmyhammer_test_utils::IsolatedTestEnvironment;` (or the appropriate re-export)
2. **Variable declarations**: Replace `let temp_dir = TempDir::new().unwrap();` with `let _guard = IsolatedTestEnvironment::new().unwrap();`
3. **Function signatures**: Update functions that take or return TempDir to use IsolatedTestEnvironment instead
4. **Path usage**: Replace `temp_dir.path()` with `_guard.temp_dir()` or use the specific directory getters like `_guard.home_path()`

### Key Benefits of IsolatedTestEnvironment:
- Provides complete HOME directory isolation 
- Creates proper `.swissarmyhammer` directory structure
- Supports parallel test execution
- Automatic cleanup via RAII
- Environment variable isolation
- Built-in retry logic for filesystem contention

### Implementation Strategy:
1. Start with files that have the most straightforward replacements
2. Handle files with custom functions that create/manage TempDir
3. Update any test utility functions to use IsolatedTestEnvironment
4. Verify all tests pass after each file conversion

The most common replacements will be:
- `TempDir::new().unwrap()` → `IsolatedTestEnvironment::new().unwrap()`
- `temp_dir.path()` → `_guard.temp_dir()` 
- For SwissArmyHammer-specific paths: use `_guard.home_path()`, `_guard.swissarmyhammer_dir()`, etc.
## Progress Update

### Files Fixed (3 of 36):
1. ✅ **tests/abort_e2e_tests.rs** - Replaced 3 TempDir usages
2. ✅ **swissarmyhammer-config/src/integration_test.rs** - Replaced 5 TempDir usages  
3. ✅ **tests/test_integration.rs** - Replaced 10 TempDir usages

**Total Progress: 18 of 262 TempDir usages replaced**

### Verified Approach:
All fixed files compile successfully with no errors. The replacement pattern is working correctly:

- `use tempfile::TempDir;` → `use swissarmyhammer_test_utils::IsolatedTestEnvironment;`
- `let temp_dir = TempDir::new().unwrap();` → `let _guard = IsolatedTestEnvironment::new().unwrap();`
- `temp_dir.path()` → `_guard.temp_dir()`
- Remove manual HOME/environment restoration code (IsolatedTestEnvironment handles this automatically)

### Key Benefits Realized:
- Tests now have proper HOME directory isolation
- Automatic `.swissarmyhammer` directory structure creation
- Better support for parallel test execution
- No more manual environment variable restoration needed
- All tests compile and ready for execution

### Next Steps:
Continue systematically replacing TempDir in remaining 33 files, focusing on:
- Files with the most straightforward patterns first
- Test utility functions that create/manage TempDir
- Verifying tests pass after each batch of changes