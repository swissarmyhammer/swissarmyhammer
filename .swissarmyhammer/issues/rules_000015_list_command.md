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
