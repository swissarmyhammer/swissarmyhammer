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
