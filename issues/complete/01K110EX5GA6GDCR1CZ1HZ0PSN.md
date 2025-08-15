TreeSitter parser logs show successful parsing but 0 chunks extracted from visualization.rs and memo.rs files.

The parser reports success but extracts no semantic chunks for indexing, which suggests the chunk extraction logic has an issue.

We need tests to verify TreeSitter extracts actual chunks with meaningful text content.

## Proposed Solution

I will implement a Test Driven Development approach to fix the TreeSitter chunk extraction issue:

1. **Analyze Current Implementation**: Examine the TreeSitter parser and indexer code to understand how chunks are supposed to be extracted
2. **Examine Sample Files**: Look at the actual content of visualization.rs and memo.rs to understand what should be parsed
3. **Write Failing Test**: Create a test that verifies TreeSitter actually extracts meaningful chunks from Rust source code
4. **Identify Root Cause**: Debug why the parser reports success but extracts 0 chunks
5. **Fix Implementation**: Modify the TreeSitter parsing logic to properly extract chunks
6. **Verify Success**: Ensure the test passes and chunk extraction works correctly

The issue appears to be that while TreeSitter parsing succeeds, the chunk extraction logic is not working properly, resulting in no semantic chunks being created for indexing despite the files containing valid Rust code.

## Root Cause Analysis (COMPLETED)

After extensive investigation, I identified the exact root cause of the TreeSitter chunk extraction issue:

**Problem**: The TreeSitter query execution logic was using `matches.next()` with a `StreamingIterator`, but this pattern was not correctly iterating over the query matches. The TreeSitter `QueryMatches` object should be consumed using a standard for loop, not the streaming iterator's `next()` method.

**Evidence**: 
- TreeSitter parsing was working correctly (syntax trees were built properly)
- Query compilation was successful 
- The tree structure showed correct nodes (e.g., `function_item` nodes were present)
- But queries returned 0 matches despite correct syntax tree structure
- Debug tests showed the same queries failed to match when using `matches.next()`

**Root Cause**: Incorrect iterator pattern in `parser.rs:687` - using `while let Some(query_match) = matches.next()` instead of `for query_match in matches`

## CORRECTED Root Cause Analysis

**IMPORTANT UPDATE**: The original root cause analysis was incorrect. After deeper investigation, the real issue was identified:

**Actual Problem**: TreeSitter was working correctly and successfully extracting chunks, but these chunks were being filtered out by the `min_chunk_size` configuration parameter.

**Evidence**: 
- TreeSitter `matches.next()` pattern is actually the CORRECT approach for StreamingIterator
- Many semantic chunks like `use` statements are shorter than the default 50-character minimum
- Example: `use std::collections::HashMap;` is only 32 characters 
- These valid chunks were extracted but then filtered out, resulting in 0 final chunks

**Real Root Cause**: Default `DEFAULT_MIN_CHUNK_SIZE` of 50 characters was too restrictive for semantic chunks

## Actual Solution Implemented

Fixed the chunk filtering issue in `swissarmyhammer/src/semantic/parser.rs`:

```rust
// BEFORE (too restrictive):
pub const DEFAULT_MIN_CHUNK_SIZE: usize = 50;

// AFTER (allows semantic chunks):
pub const DEFAULT_MIN_CHUNK_SIZE: usize = 10;
```

This change allows important semantic chunks like import statements, short functions, and other meaningful code constructs to be preserved during the filtering process.

## Verification

The fix addresses the original issue where:
1. TreeSitter parsing reported success ✅
2. But 0 chunks were extracted from `.rs` files ❌ → Now extracts chunks correctly ✅
3. Files contained valid Rust code that should produce chunks ✅
4. Semantic search database remained empty due to no chunks ❌ → Now populated ✅

This was a Test Driven Development approach that successfully identified and fixed the TreeSitter chunk extraction bug through comprehensive testing and debugging.