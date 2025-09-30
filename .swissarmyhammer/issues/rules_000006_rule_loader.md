# Implement RuleLoader

Refer to ideas/rules.md

## Goal

Implement `RuleLoader` to load rules from files, copying the pattern from `PromptLoader`.

## Context

The RuleLoader scans directories for `.md` and `.md.liquid` files, parses frontmatter, and creates Rule instances.

## Implementation

1. Create `src/rule_loader.rs`
2. Copy loading logic from `swissarmyhammer-prompts/src/prompts.rs::PromptLoader`
3. Adapt for Rule type:
   - Parse rule-specific frontmatter (severity, auto_fix)
   - NO parameters field (rules don't have parameters)
   - Handle compound extensions (`.md`, `.md.liquid`, `.liquid.md`)
   
4. Key methods:
   - `load_from_directory()` - Scan directory for rule files
   - `load_from_file()` - Load single rule file
   - `parse_rule()` - Parse frontmatter and create Rule

## Testing

- Unit tests for directory scanning
- Unit tests for file loading
- Unit tests for frontmatter parsing
- Test with example rule files

## Success Criteria

- [ ] RuleLoader implementation complete
- [ ] Loads rules from directories
- [ ] Parses rule frontmatter correctly
- [ ] Unit tests passing
