# Implement Rule Evaluation Caching

## Description
Add caching for rule evaluation results to avoid re-checking unchanged file/rule pairs. This will significantly speed up repeated rule checks.

## Requirements

### Cache Key
- Hash of the file content + rule content pair
- Should uniquely identify a specific file content evaluated against a specific rule version
- Suggested approach: `SHA256(file_content + rule_content)` or similar

### Cache Location
- Store cache in `~/.cache/swissarmyhammer/`
- Follow XDG Base Directory specification on Linux/macOS
- Each cache entry should be an individual file

### Cache Structure
Each cache entry file should contain:
- Result: PASS or VIOLATION
- Timestamp (when cached)
- If violation: the full violation details

Suggested filename: `{hash}.cache` or similar

### Cache Behavior
1. Before checking a file against a rule:
   - Calculate hash of file content + rule content
   - Check if cache entry exists for that hash
   - If exists and valid, return cached result (skip LLM call)
   - If not exists, perform check and cache result

2. Cache invalidation:
   - Cache is automatically invalidated when file or rule content changes (different hash)
   - No automatic cleanup or size limits
   - Manual clearing only via `sah rule cache clear` command

### New Command
Add `sah rule cache clear` subcommand:
```bash
sah rule cache clear
```

This should:
- Remove all files from `~/.cache/swissarmyhammer/`
- Report number of cache entries cleared
- Handle case where cache directory doesn't exist gracefully

## Implementation Notes

### Cache Module
Create `swissarmyhammer-rules/src/cache.rs`:
- `RuleCache` struct
- `calculate_cache_key(file_content: &str, rule_content: &str) -> String`
- `get_cached_result(key: &str) -> Option<CachedResult>`
- `store_result(key: &str, result: CachedResult)`
- `clear_cache() -> Result<usize>` (returns number of entries cleared)

### Integration with RuleChecker
In `checker.rs`, modify `check_file()`:
1. Calculate cache key from file content + rendered rule
2. Check cache before calling LLM
3. Store result in cache after LLM call

### CLI Integration
Add `cache` subcommand to `sah rule` in `commands/rule.rs`:
```rust
enum RuleCommand {
    Check { ... },
    List { ... },
    Cache { action: CacheAction },
}

enum CacheAction {
    Clear,
}
```

## Benefits
- Dramatically faster repeated checks when files/rules unchanged
- Reduces LLM API costs
- Improves developer experience for iterative development

## Acceptance Criteria
- [ ] Cache key is hash of file + rule content pair
- [ ] Cache stored in `~/.cache/swissarmyhammer/`
- [ ] Each cache entry is an individual file
- [ ] Cache is checked before LLM evaluation
- [ ] Results are stored in cache after LLM evaluation
- [ ] Cache is automatically invalidated when file/rule changes (via hash)
- [ ] `sah rule cache clear` command clears all cache entries
- [ ] No automatic cleanup or size limits on cache
- [ ] Tests verify caching behavior works correctly
- [ ] Tests verify cache clear command works
- [ ] Documentation for cache behavior



## Proposed Solution

Based on the code analysis, I will implement the caching feature as follows:

### 1. Cache Module (`swissarmyhammer-rules/src/cache.rs`)
- Create `RuleCache` struct with methods for cache operations
- Use SHA-256 hash of `file_content + rule.template` as cache key
- Store cache in `~/.cache/swissarmyhammer/rules/` directory
- Each cache entry is a JSON file named `{hash}.cache`
- Cache entry structure:
  ```json
  {
    "result": "PASS" or "VIOLATION",
    "timestamp": "2025-10-05T12:00:00Z",
    "violation_details": "optional violation message"
  }
  ```

### 2. Integration into RuleChecker
Modify `checker.rs` `check_file()` method:
1. Before Stage 1 rendering, calculate cache key from file content + rule template
2. Check cache for existing result
3. If cache hit: return cached result immediately (skip LLM call)
4. If cache miss: proceed with normal flow, then store result in cache

### 3. CLI Command Extension
Add `Cache` variant to `RuleCommand` enum in `cli.rs`:
```rust
pub enum RuleCommand {
    List(ListCommand),
    Validate(ValidateCommand),
    Check(CheckCommand),
    Cache(CacheCommand),  // NEW
}

pub enum CacheAction {
    Clear,
}

pub struct CacheCommand {
    pub action: CacheAction,
}
```

Update `parse_rule_command` and `run_rule_command_typed` to handle new cache subcommand.

### 4. Dependencies
Need to add to `swissarmyhammer-rules/Cargo.toml`:
- `sha2` for SHA-256 hashing
- `serde` and `serde_json` for cache serialization (already present)
- Use existing `dirs` or similar for XDG cache directory

### 5. Testing Strategy
- Unit tests for hash calculation consistency
- Unit tests for cache storage and retrieval
- Integration test: check file twice, verify second call uses cache
- Test cache clear command
- Test cache invalidation (file content change, rule change)

### Implementation Order
1. Create cache.rs with basic structure
2. Implement hash function and storage
3. Add tests for cache module
4. Integrate into RuleChecker
5. Add CLI command
6. Add integration tests
7. Run full test suite




## Implementation Complete

Successfully implemented rule evaluation caching with the following:

### Files Created
- `swissarmyhammer-rules/src/cache.rs` - Core cache module with RuleCache struct
- `swissarmyhammer-cli/src/commands/rule/cache.rs` - CLI command handler for cache operations

### Files Modified
- `swissarmyhammer-rules/src/lib.rs` - Added cache module and public exports
- `swissarmyhammer-rules/src/error.rs` - Added CacheError variant to RuleError
- `swissarmyhammer-rules/src/checker.rs` - Integrated cache into check_file method
- `swissarmyhammer-rules/Cargo.toml` - Added sha2 dependency
- `swissarmyhammer-cli/src/commands/rule/mod.rs` - Added cache module and command routing
- `swissarmyhammer-cli/src/commands/rule/cli.rs` - Added CacheCommand and CacheAction types

### Key Implementation Details

1. **Cache Key Generation**: SHA-256 hash of `file_content + rule.template` ensures automatic invalidation when either changes

2. **Cache Storage**: 
   - Location: `~/.cache/swissarmyhammer/rules/`
   - Format: JSON files named `{hash}.cache`
   - Structure includes result, timestamp, and violation details if applicable

3. **Cache Integration**:
   - Check cache before LLM evaluation
   - On cache hit: return cached result immediately (with appropriate logging)
   - On cache miss: proceed with LLM check, then store result
   - Cache failures are logged as warnings but don't interrupt normal flow

4. **CLI Command**: `sah rule cache clear` removes all cache entries

5. **Serialization Strategy**: 
   - Created `CachedResultData` as a serializable intermediate type
   - `CachedResult` enum wraps `RuleViolation` for public API
   - Avoids need to add Serialize/Deserialize to RuleViolation

### Test Results
- All 3265 tests passed
- Cache module includes 11 comprehensive unit tests covering:
  - Key calculation consistency
  - Storage and retrieval (Pass and Violation)
  - Cache miss behavior
  - Cache clearing
  - Automatic invalidation on file/rule changes
  - Timestamp tracking

### Performance Impact
- Cache hits skip expensive LLM calls entirely
- Significantly faster for repeated checks of unchanged files
- Reduces API costs for iterative development workflows

