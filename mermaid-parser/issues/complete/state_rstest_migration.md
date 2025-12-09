# Migrate State tests to rstest file-based pattern

## Description
The State parser tests (`tests/state_test.rs`) exist but only use regular unit tests. They need to be migrated to use rstest file-based testing pattern for consistency and comprehensive validation.

## Current State
- Parser implementation: ✅ Complete (recently implemented with hierarchical support)
- Unit tests in parser file: ✅ Present
- Test file exists: ✅ `tests/state_test.rs` (regular tests only)
- Test data files: ❌ Missing `test/state/` directory
- File-based rstest: ❌ Not implemented

## Requirements
1. Create `test/state/` directory
2. Copy state diagram test files from mermaid-samples or create representative examples
3. Update `tests/state_test.rs` to use rstest file-based pattern
4. Maintain existing unit tests if they provide unique value
5. Validate state machine features: states, transitions, composite states

## Implementation Steps
1. Create test data directory: `mkdir -p test/state/`
2. Populate with state diagram examples
3. Add rstest dependency and file-based test function
4. Ensure coverage of hierarchical state features

## Special Considerations
- State diagrams are Phase 3 "Advanced" complexity
- Support for hierarchical/composite states
- Various state types (start, end, fork, join)
- Transition conditions and actions
- Recent implementation with hierarchical support needs validation

## Success Criteria
- Test data directory created and populated
- File-based tests validate all state diagram features
- Hierarchical state support thoroughly tested
- All test files parse successfully