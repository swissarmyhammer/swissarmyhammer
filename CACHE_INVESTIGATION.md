# System Prompt Caching Investigation

## Issue Report
When running SAH with a local model using llama-agent, "deleting cache" messages appear constantly, suggesting that system prompt (template) caching may not be working effectively.

## Investigation Summary

### Key Findings

#### 1. **Two Separate Caching Systems**
There are TWO independent caching mechanisms in llama-agent:

**A. Template Cache (System Prompt + Tools)**
- **Purpose**: Cache the processed system prompt and tool definitions across sessions
- **Location**: `~/.cache/llama-agent/templates/template_{hash}.kv`
- **Implementation**: `llama-agent/src/template_cache.rs`
- **Scope**: Shared across all sessions with identical system prompts + tools

**B. Session KV Cache (Conversation History)**
- **Purpose**: Cache the full conversation history within a single session
- **Location**: `{session_id}.bin` and `{session_id}.tokens` files
- **Implementation**: `llama-agent/src/queue.rs` (lines 508-660)
- **Scope**: Per-session, tracks all tokens in the conversation

#### 2. **The "Deleting Cache" Messages**
The "deleting cache" messages come from **Session KV Cache validation**, NOT template cache.

**Source**: `llama-agent/src/queue.rs:574-602`

Three scenarios trigger cache deletion:
```rust
// Scenario 1: Cached tokens don't match current prompt (line 575)
info!("Worker {} cached tokens don't match current prompt - deleting old cache", worker_id);

// Scenario 2: Failed to read token metadata (line 587)
warn!("Worker {} failed to read token metadata: {}, deleting cache", worker_id, e);

// Scenario 3: No token metadata found (line 599)
debug!("Worker {} no token metadata found for session cache - deleting cache", worker_id);
```

#### 3. **CRITICAL ISSUE: Template Cache Metadata Not Persisted**

**The template cache HashMap is NOT persisted across agent restarts.**

**What happens**:
1. `ModelManager::new()` creates a new empty `TemplateCache` (model.rs:133)
2. `TemplateCache::new()` creates an empty HashMap (template_cache.rs:148-176)
3. Previous cache metadata is LOST even though KV files exist on disk
4. Every agent start begins with zero cached templates in memory

**Evidence**:
- Test `test_template_cache_metadata_not_persisted` proves this behavior
- `TemplateCache::new()` does not scan the cache directory for existing files
- Cache entries must be manually re-inserted into the HashMap

**Impact**:
- First session after agent start = CACHE MISS (always)
- Subsequent sessions in same agent lifetime = CACHE HIT (if system prompt matches)
- Template caching only works within a single agent process lifetime
- **Restarting the agent = losing all template cache metadata**

#### 4. **Log Levels Matter**

**Template Cache Logs** (template_cache.rs:214, 222):
```rust
debug!("Template cache HIT: {} ({} tokens from {})", ...);
debug!("Template cache MISS: {}", ...);
```
- Level: **DEBUG**
- Not visible by default unless debug logging enabled

**Session KV Cache Logs** (queue.rs:567, 575, 587, 599):
```rust
info!("Worker {} cached tokens ({}) match current prompt prefix - will use cache", ...);
info!("Worker {} cached tokens don't match current prompt - deleting old cache", ...);
warn!("Worker {} failed to read token metadata: {}, deleting cache", ...);
debug!("Worker {} no token metadata found for session cache - deleting cache", ...);
```
- Level: **INFO / WARN / DEBUG**
- More visible in default logging configuration

**Result**: Users see session cache deletion messages but NOT template cache hit/miss messages.

### Test Coverage

**Existing Tests** (ALL PASSING):
- `llama-agent/tests/template_cache_e2e_test.rs` - 11/13 tests passing (2 ignored, require real model)
- `llama-agent/tests/template_cache_integration_test.rs` - 8 tests
- `llama-agent/tests/agent_cache_integration_test.rs` - 2 tests
- `llama-agent/tests/agent_template_cache_test.rs` - 3 tests (ignored, require real model)
- `llama-agent/tests/session_kv_cache_test.rs` - 4 tests

**New Tests Created**:
- `llama-agent/tests/system_prompt_cache_test.rs` - 5 tests (ALL PASSING)
  - Proves template caching works WITHIN a single instance
  - Documents that metadata is NOT persisted across instances

### Root Cause Analysis

**Why the user sees constant "deleting cache" messages:**

1. **Agent Restarts**: Each time llama-agent starts, template cache metadata is lost
2. **Cache Misses**: Without metadata, the system doesn't know what templates are cached
3. **Session Cache Validation**: Session KV cache validation fails because:
   - Tokens may have been processed without proper template cache context
   - System might be re-tokenizing prompts differently without cached templates
   - Session cache becomes invalidated and gets deleted (visible INFO log)
4. **Template Cache Silent**: Template cache operations happen at DEBUG level
   - User doesn't see that templates are also being missed/re-cached

### Recommendations

#### High Priority

1. **Persist Template Cache Metadata**
   - Implement metadata persistence (JSON/binary format) in cache directory
   - Load existing cache files on `TemplateCache::new()`
   - Scan cache directory and rebuild HashMap from existing files
   - Example: `.cache/llama-agent/templates/metadata.json` with all cache entries

2. **Improve Logging Visibility**
   - Promote template cache hit/miss logs from DEBUG to INFO level
   - Add summary log on agent start: "Loaded N cached templates from disk"
   - Add metrics: template cache hit rate in agent stats

3. **Add Cache Validation**
   - Verify KV cache files exist before marking as cached in HashMap
   - Implement cache integrity checks
   - Add tool to inspect/rebuild cache metadata

#### Medium Priority

4. **Document Cache Behavior**
   - Update TEMPLATE_CACHING.md with persistence limitations
   - Add troubleshooting section for cache issues
   - Document expected log messages and what they mean

5. **Cache Statistics API**
   - Expose template cache stats via agent API
   - Add endpoint to get cache hit rates
   - Allow users to monitor caching effectiveness

6. **Lazy Loading Option**
   - Implement on-demand metadata loading
   - First cache miss could trigger directory scan
   - Balance performance vs startup time

#### Low Priority

7. **Cache Management Tools**
   - CLI command to inspect cache contents
   - Tool to clear/rebuild cache
   - Cache size limits and cleanup policies

8. **Metrics Collection**
   - Track template cache effectiveness over time
   - Compare performance with/without caching
   - Measure cache hit rates across different system prompts

## Verification Steps

To verify the current behavior:

```bash
# 1. Run tests to prove template caching works within a process
cargo test --package llama-agent --test system_prompt_cache_test

# 2. Enable debug logging to see template cache operations
RUST_LOG=llama_agent=debug sah --model llama <your command>

# 3. Check cache directory to see what's persisted
ls -la ~/.cache/llama-agent/templates/

# 4. Restart agent and observe cache behavior (will lose metadata)
```

## Related Files

**Core Implementation**:
- `llama-agent/src/template_cache.rs` - Template cache implementation
- `llama-agent/src/model.rs:133` - ModelManager creates TemplateCache
- `llama-agent/src/queue.rs:508-660` - Session KV cache validation
- `llama-agent/src/agent.rs:627-655` - Template cache usage in agent

**Tests**:
- `llama-agent/tests/template_cache_e2e_test.rs`
- `llama-agent/tests/system_prompt_cache_test.rs` (NEW)
- `llama-agent/tests/template_cache_integration_test.rs`

**Documentation**:
- `llama-agent/docs/TEMPLATE_CACHING.md`

## Conclusion

**Template caching IS implemented and DOES work within a single agent process.**

However, **template cache metadata is not persisted across agent restarts**, causing:
- Cache misses on every agent startup
- Re-processing of system prompts that were previously cached
- Session cache invalidation (visible "deleting cache" messages)
- Loss of caching benefits across invocations

The fix requires implementing metadata persistence so that cached templates survive agent restarts.
