# Implement RuleLibrary and Build Script

Refer to ideas/rules.md

## Goal

Implement `RuleLibrary` for rule collection management and create build script to embed builtin rules.

## Context

RuleLibrary manages a collection of rules with add/get/list/search operations. The build script embeds builtin rules in the binary.

## Implementation

1. In `src/rules.rs`, implement `RuleLibrary`:
   - Collection management (add/get/list/remove)
   - Search and filtering
   - NO rendering (rules don't render themselves)
   
2. Create `build.rs`:
   - Copy pattern from `swissarmyhammer-prompts/build.rs`
   - Embed `builtin/rules/` directory
   - Generate code to include builtin rules

3. Create `builtin/rules/` directory structure:
   - `builtin/rules/security/`
   - `builtin/rules/code-quality/`
   - `builtin/rules/_partials/`

4. Add basic README explaining builtin rules

## Testing

- Unit tests for library operations
- Test builtin rules are embedded correctly
- Integration test loading builtin rules

## Success Criteria

- [ ] RuleLibrary implementation complete
- [ ] build.rs embeds builtin rules
- [ ] builtin/rules/ directory created
- [ ] Library operations tested
- [ ] Builtin rules accessible
