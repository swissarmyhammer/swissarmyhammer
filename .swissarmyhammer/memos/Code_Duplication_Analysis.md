# Code Duplication Analysis for search/index/mod.rs

## Analysis Focus
Looking for:
- Identical or near-identical code blocks (>5 lines)
- Similar algorithms that could be abstracted
- Repeated constant values or configuration
- Duplicate test setup or assertion patterns

## Findings

### 1. Duplicate Test File Creation Pattern
**Location**: Lines 369-386 and Lines 435-449
**Similarity**: Both test functions create temporary directories and write test Rust files with similar structure

### 2. Duplicate Error Handling in Tests
**Location**: Lines 391-412 and throughout test suite
**Similarity**: Similar error handling pattern checking for model initialization failures

### 3. Repeated Test Context Setup
**Location**: Multiple test functions (lines 342, 360, 423, 509, 627)
**Similarity**: All tests call `create_test_context().await` with identical pattern

### 4. Duplicate Progress Notification Collection
**Location**: Lines 467-470 and Lines 668-671
**Similarity**: Identical pattern for collecting notifications from channel

### 5. Repeated Configuration Initialization Pattern
**Location**: Lines 133-148
**Similarity**: Nested conditional compilation blocks for test vs. non-test config, repeated pattern for indexer creation at lines 150-168
