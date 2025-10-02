# Implement Rule List Command

Refer to ideas/rules.md

## Goal

Implement the `sah rule list` command with filtering and multiple output formats.

## Context

The list command shows all available rules from all sources (builtin/user/local) with emoji-based source indicators.

## Implementation

1. In `list.rs`, implement `execute_list_command()`:
   - Load all rules from all sources via RuleResolver
   - Build RuleFilter (no filtering for basic list)
   - Get file sources for emoji display
   - Convert to display rows
   - Support table/JSON/YAML output via cli_context

2. Follow exact pattern from `prompt list` command
3. Filter out partial templates (if rules support them)
4. Support verbose mode with --verbose flag

5. CLI arguments:
   - `--verbose` - Show detailed information
   - `--format [table|json|yaml]` - Output format

## Testing

- Test list with no rules
- Test list with rules from all sources
- Test output formats
- Test verbose mode

## Success Criteria

- [ ] List command implemented
- [ ] Shows rules from all sources
- [ ] Emoji sources display correctly
- [ ] All output formats work
- [ ] Tests passing



## Proposed Solution

After analyzing the existing code, I can see that:

1. The `list.rs` file already exists but has a TODO comment about using RuleResolver
2. The `display.rs` file is complete with correct emoji mapping and display structures
3. The CLI structure is already in place

The implementation needs to:

1. **Update `list.rs` to use RuleResolver**:
   - Load rules from all sources (builtin/user/local) using RuleResolver
   - Get file sources for emoji display (📦 Built-in, 📁 Project, 👤 User)
   - Filter out partial templates (if any exist)
   - Use the existing display infrastructure

2. **Follow the exact pattern from `prompt list` command**:
   - Use `RuleLibrary` and `RuleResolver` for loading
   - Build `RuleFilter` for filtering
   - Convert FileSource to RuleSource for library API
   - Filter partial templates
   - Pass to display infrastructure

3. **Add comprehensive tests** matching the prompt list tests:
   - Test with all output formats (table/JSON/YAML)
   - Test verbose mode
   - Test debug mode
   - Test quiet mode
   - Test all combinations

The display infrastructure is already complete and tested, so the main work is updating the list command implementation to use RuleResolver properly.



## Implementation Notes

Successfully implemented the `sah rule list` command following the exact pattern from `prompt list`:

### Changes Made

1. **Updated `list.rs`**:
   - Replaced simple `RuleLibrary::new()` with `RuleResolver` pattern
   - Load rules from all sources (builtin → user → local) via `resolver.load_all_rules()`
   - Get file sources from `resolver.rule_sources` for emoji display
   - Filter out partial templates using `rule.is_partial()` method
   - Pass rules to display infrastructure with source information

2. **Key Differences from Prompts**:
   - Rules use `is_partial()` instead of `is_partial_template()`
   - RuleResolver API uses `Vec<Rule>` instead of `RuleLibrary`
   - No separate `RuleSource` conversion needed - can use `FileSource` directly

3. **Tests Added**:
   - All integration tests for different output formats (table/JSON/YAML)
   - Verbose mode tests
   - Debug and quiet mode tests
   - Unit tests for partial template filtering logic
   - Test helpers matching prompt list test structure

### Test Results

All 30 tests passing:
- Integration tests with all output formats ✓
- Verbose mode tests ✓
- Filtering logic tests ✓
- Edge case tests ✓

The implementation correctly:
- Loads rules from all three sources with proper precedence
- Displays emoji-based source indicators (📦 Built-in, 📁 Project, 👤 User)
- Filters out partial templates
- Supports all output formats via cli_context
- Handles verbose mode for detailed output
