# Investigate Rule Cache File Persistence

## Problem

Rule caching appears to work during execution (logs show "Cache hit" and "Cached result for key"), but cache files don't persist in `~/.cache/swissarmyhammer/rules/` after the program exits.

## Symptoms

1. **During Execution**: Cache works correctly
   - "Cache hit" messages appear in logs with timestamps
   - "Cached result for key: {hash}" messages show writes happening
   - Second run of same rule shows cache hits

2. **After Execution**: No cache files found
   - `ls ~/.cache/swissarmyhammer/rules/` returns empty
   - No `.cache` files exist
   - Directory exists and is writable (test file creation works)

## Investigation Needed

### 1. Verify Cache File Paths
Check if cache files are being written to the expected location:
- Expected: `~/.cache/swissarmyhammer/rules/{hash}.cache`
- Code: `swissarmyhammer-rules/src/cache.rs:293-294`

### 2. Check for Silent Write Failures
Add instrumentation to verify `fs::write()` actually succeeds:
- Code: `cache.rs:244` - `fs::write(&cache_file, content)`
- Check if errors are being swallowed somewhere

### 3. Check for Cleanup on Exit
Search for code that might be deleting cache files:
- Tempfile cleanup?
- Drop implementations?
- Test cleanup code running in production?

### 4. Verify Directory Permissions
The directory is writable (manual `touch` test works), but verify:
- Permissions are correct after `fs::create_dir_all()`
- Parent directory ownership
- macOS sandboxing or security settings

## Reproduction Steps

```bash
# Clear cache
rm -rf ~/.cache/swissarmyhammer/rules
mkdir -p ~/.cache/swissarmyhammer/rules

# Run rule check
cargo run -- --debug rule check --rule no-mocks README.md 2>&1 | grep -E "Cached result|Cache hit"

# Check for cache files immediately after
ls -la ~/.cache/swissarmyhammer/rules/

# Expected: Should see .cache files
# Actual: Directory is empty
```

## Code Locations

- Cache implementation: `swissarmyhammer-rules/src/cache.rs`
- Cache directory: `cache.rs:145-150` - `~/.cache/swissarmyhammer/rules/`
- Cache write: `cache.rs:232-250` - `store()` method
- Cache read: `cache.rs:202-220` - `get()` method

## Success Criteria

- [ ] Cache files persist after program exit
- [ ] Cache files are readable on subsequent runs
- [ ] Cache directory location is documented and correct
- [ ] No silent failures in cache writing
- [ ] Cache behaves consistently across multiple runs
