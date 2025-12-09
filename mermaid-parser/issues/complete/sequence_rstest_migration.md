# Migrate Sequence tests to rstest file-based pattern

## Description
The Sequence parser tests (`tests/sequence_test.rs`) exist but only use regular unit tests. They need to be migrated to use rstest file-based testing pattern for consistency and comprehensive validation.

## Current State
- Parser implementation: ✅ Complete
- Unit tests in parser file: ✅ Present
- Test file exists: ✅ `tests/sequence_test.rs` (regular tests only)
- Test data files: ❌ Missing `test/sequence/` directory
- File-based rstest: ❌ Not implemented

## Requirements
1. Create `test/sequence/` directory
2. Copy sequence diagram test files from mermaid-samples or create representative examples
3. Update `tests/sequence_test.rs` to use rstest file-based pattern
4. Maintain existing unit tests if they provide unique value
5. Validate complex sequence diagram features: actors, messages, loops, alternatives

## Implementation Steps
1. Create test data directory: `mkdir -p test/sequence/`
2. Populate with sequence diagram examples (55 available from samples)
3. Add rstest dependency and file-based test function
4. Ensure comprehensive coverage of sequence diagram features

## Special Considerations
- Sequence diagrams are Phase 3 "Advanced" complexity
- Support for various message types (sync, async, return)
- Complex control flow (loop, alt, opt, par)
- Actor and participant management
- Note and activation handling

## Success Criteria
- Test data directory created and populated
- File-based tests validate all sequence diagram features
- Existing valuable unit tests preserved
- All test files parse successfully