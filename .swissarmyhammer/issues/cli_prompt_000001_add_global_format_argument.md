# Add Global --format Argument to Root CLI

Refer to /Users/wballard/github/swissarmyhammer/ideas/cli_prompt.md

## Overview

Add `--format` as a global argument to the root CLI and expand CliContext to support global output formatting for prompt commands. This establishes the foundation for the CliContext pattern.

## Current State

- `--format` is currently defined per-subcommand in prompt list
- CliContext exists but only wraps TemplateContext
- No global output formatting support

## Goals

- Move `--format` from prompt subcommand to root CLI level
- Expand CliContext to include global format setting
- Enable `sah --format=json prompt list` syntax
- Leave other commands unchanged for now

## Implementation Steps

### 1. Update Root CLI Definition

**File**: `swissarmyhammer-cli/src/cli.rs`

- Add `--format` flag to root `Cli` struct
- Keep existing per-subcommand format flags for backward compatibility temporarily
- Update help text to reflect global option availability

### 2. Expand CliContext Structure

**File**: `swissarmyhammer-cli/src/context.rs`

```rust
pub struct CliContext {
    pub template_context: TemplateContext,
    pub format: OutputFormat,
    pub verbose: bool,
    pub debug: bool,
    pub quiet: bool,
}

impl CliContext {
    pub fn display<T>(&self, items: Vec<T>) -> Result<(), DisplayError>
    where
        T: Tabled + Serialize
    {
        match self.format {
            OutputFormat::Table => { /* table logic */ }
            OutputFormat::Json => { /* json logic */ }
            OutputFormat::Yaml => { /* yaml logic */ }
        }
    }
}
```

### 3. Update Main.rs Argument Parsing

**File**: `swissarmyhammer-cli/src/main.rs`

- Parse global `--format` flag in `handle_dynamic_matches()`
- Pass format to CliContext constructor
- Ensure prompt commands receive CliContext instead of just TemplateContext

### 4. Update CliContext Constructor

**File**: `swissarmyhammer-cli/src/context.rs`

- Update `CliContext::new()` to accept format parameter
- Extract format from matches or use default

## Testing Requirements

### Unit Tests
- Test global `--format` flag parsing
- Test CliContext creation with different format options
- Test display() method with different output formats

### Integration Tests  
- Test `sah --format=json prompt list`
- Test `sah --format=yaml prompt list`
- Test backward compatibility with subcommand format flags

## Success Criteria

1. ✅ Global `--format` argument available at root CLI level
2. ✅ CliContext properly constructed with global format setting
3. ✅ Prompt commands can access global format via CliContext
4. ✅ All existing functionality preserved
5. ✅ Tests pass for new functionality

## Files Modified

- `swissarmyhammer-cli/src/cli.rs` - Add global --format
- `swissarmyhammer-cli/src/context.rs` - Expand CliContext  
- `swissarmyhammer-cli/src/main.rs` - Update argument parsing

## Risk Mitigation

- Keep existing per-subcommand flags during transition
- Extensive testing to prevent regressions
- Gradual rollout to prompt commands only initially

---

**Estimated Effort**: Small (< 100 lines changed)
**Dependencies**: None
**Blocks**: All subsequent prompt command refactoring steps