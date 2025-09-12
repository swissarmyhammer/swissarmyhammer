# Remove Duplicate Prompt Validate Command

## Problem

There are currently TWO different validate commands that create confusion and duplication:

1. **`sah validate`** - Standalone command that validates prompts AND workflows
2. **`sah prompt validate`** - Subcommand that validates prompts only

This creates:
- **User Confusion**: Which validate command should users use?
- **Code Duplication**: Two different validation code paths
- **Maintenance Burden**: Changes need to be made in multiple places
- **Architectural Inconsistency**: Why is validate both standalone and under prompt?

## Current State

**Standalone validate**: `src/commands/validate/mod.rs`
- Validates prompts AND workflows
- Has comprehensive validation logic
- Well-documented and tested
- Proper command structure

**Prompt subcommand validate**: `src/commands/prompt/mod.rs`
- Only validates prompts
- Simpler implementation
- Appears to be redundant with standalone command

## Proposed Solution

**Remove `sah prompt validate` entirely** and keep only `sah validate`:

### Rationale

1. **Single Purpose**: Validation is a cross-cutting concern, not prompt-specific
2. **More Powerful**: `sah validate` already handles prompts plus workflows
3. **Less Confusion**: One validation command, clear usage
4. **Better UX**: `sah validate` is shorter and more discoverable than `sah prompt validate`

### User Migration

**Before**:
```bash
sah prompt validate my-prompt.md
```

**After**:
```bash
sah validate my-prompt.md                # Validate specific prompt
sah validate                             # Validate all prompts and workflows
sah validate --prompts                   # Validate only prompts (if needed)
```

## Implementation Steps

### 1. Remove from Prompt Commands

**File**: `swissarmyhammer-cli/src/commands/prompt/mod.rs`
- Remove `PromptCommand::Validate` handling
- Remove `run_validate_command()` function
- Update routing to only handle List and Test

**File**: `swissarmyhammer-cli/src/commands/prompt/cli.rs`
- Remove `ValidateCommand` struct
- Remove `PromptCommand::Validate` enum variant
- Update parsing to not recognize validate subcommand

### 2. Update Documentation

**Files to update**:
- `doc/src/03-prompts/prompts.md` - Update examples to use `sah validate`
- `doc/src/07-reference/cli-reference.md` - Remove prompt validate references
- Any other docs that mention `sah prompt validate`

### 3. Update Tests

**Files to update**:
- Remove any tests that specifically test `sah prompt validate`
- Update integration tests to use `sah validate` instead
- Ensure standalone validate command covers all prompt validation use cases

### 4. Update Help Text

**File**: `src/commands/prompt/description.md`
- Remove any references to validate subcommand
- Point users to `sah validate` for validation needs

## Validation

### Before Removal
- Verify that `sah validate` handles all prompt validation use cases
- Ensure no functionality is lost by removing the subcommand
- Confirm all tests pass with standalone validate only

### After Removal  
- All prompt validation functionality available via `sah validate`
- No broken references in documentation
- Clean prompt command interface with only list/test

## Success Criteria

1. ✅ `sah prompt validate` command no longer exists
2. ✅ `sah validate` handles all prompt validation use cases
3. ✅ Documentation updated to use correct validate command
4. ✅ No functionality lost in the removal
5. ✅ Clean prompt command interface: only list and test
6. ✅ All tests pass with updated command usage

## Files Modified

- `swissarmyhammer-cli/src/commands/prompt/mod.rs` - Remove validate handling
- `swissarmyhammer-cli/src/commands/prompt/cli.rs` - Remove validate command
- Documentation files - Update validate command usage
- Test files - Update to use standalone validate

---

**Priority**: High - Reduces architectural confusion  
**Estimated Effort**: Medium (removal + doc updates)
**Dependencies**: None (simplification)
**Blocks**: Clean prompt command architecture