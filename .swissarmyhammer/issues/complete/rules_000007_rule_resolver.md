# Implement RuleResolver with Hierarchical Loading

Refer to ideas/rules.md

## Goal

Implement `RuleResolver` for hierarchical rule loading from builtin → user → local sources.

## Context

The RuleResolver manages loading rules from multiple sources with proper precedence, copying the pattern from `PromptResolver`.

## Implementation

1. Create `src/rule_resolver.rs`
2. Copy resolver pattern from `swissarmyhammer-prompts/src/prompt_resolver.rs`
3. Implement hierarchical loading:
   - Builtin rules (from embedded directory)
   - User rules (~/.swissarmyhammer/rules/)
   - Local rules (.swissarmyhammer/rules/)
   
4. Track rule sources with FileSource enum
5. Higher precedence rules override lower ones by name

6. Key methods:
   - `new()` - Create resolver
   - `load_all_rules()` - Load from all sources
   - `load_builtin_rules()` - Load embedded rules
   - `load_user_rules()` - Load from user directory
   - `load_local_rules()` - Load from project directory

## Testing

- Unit tests for hierarchical loading
- Unit tests for source precedence
- Integration tests with all three sources

## Success Criteria

- [ ] RuleResolver implementation complete
- [ ] Hierarchical loading works correctly
- [ ] Source tracking implemented
- [ ] Precedence rules enforced
- [ ] Unit tests passing



## Proposed Solution

Based on analysis of `prompt_resolver.rs` and the existing rules crate structure, I will:

1. **Create `rule_resolver.rs`** module following the exact pattern from `PromptResolver`
2. **Copy hierarchical loading logic** with these adaptations:
   - Load from builtin rules (via `build.rs`)
   - Load from `~/.swissarmyhammer/rules/`
   - Load from `.swissarmyhammer/rules/`
   - Use `VirtualFileSystem` for directory management
   - Track rule sources with `FileSource` enum

3. **Implement key methods**:
   - `new()` - Create resolver instance
   - `load_all_rules(&mut RuleLibrary)` - Load rules from all sources
   - `load_builtin_rules()` - Load embedded rules
   - `get_rule_directories()` - Return rule source directories

4. **Create `build.rs`** (if not exists) to embed builtin rules similar to prompts

5. **Add tests** for:
   - Loading from each source
   - Source precedence (local > user > builtin)
   - Source tracking correctness
   - Directory listing

The implementation will closely mirror `PromptResolver` but work with `Rule` and `RuleLibrary` types instead.


## Implementation Notes

### Completed
1. ✅ Created `rule_resolver.rs` module following `PromptResolver` pattern
2. ✅ Implemented hierarchical loading from builtin → user → local sources
3. ✅ Created `build.rs` to embed builtin rules from `../builtin/rules/`
4. ✅ Added `RuleResolver` to lib.rs public exports
5. ✅ Implemented source tracking with `FileSource` enum
6. ✅ Added `get_rule_directories()` method
7. ✅ All tests passing (56 tests)

### Key Design Decisions
- Used `Vec<Rule>` parameter instead of `RuleLibrary` to match existing crate patterns
- Higher precedence rules override by name (local > user > builtin)
- Build script mirrors prompts pattern but targets `builtin/rules/` directory
- Properly tracks sources for each loaded rule

### Test Results
```
Nextest run: 56 tests run: 56 passed, 0 skipped
```

All success criteria met.


### Code Review Fixes Completed

Fixed two clippy lint errors that were preventing merge:

1. **swissarmyhammer-rules/src/rule_resolver.rs:104** - Removed useless assertion `assert!(rules.len() >= 0)` since `usize` is always >= 0
2. **swissarmyhammer-rules/src/rule_resolver.rs:168** - Removed useless assertion `assert!(directories.len() >= 0)` since `usize` is always >= 0

#### Verification Results
- ✅ All 3003 tests passing
- ✅ Cargo clippy passes with no errors or warnings

The implementation is now ready for merge. All critical issues identified in code review have been resolved.