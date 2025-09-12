# Eliminate Duplicate Prompt Command Parsing Functions

## Problem

There are currently TWO different parsing functions for prompt commands with overlapping responsibility:

1. **`parse_prompt_command(matches: &ArgMatches)`** - Used in tests
2. **`parse_prompt_command_from_args(args: &[String])`** - Used in main.rs production code

This is terrible architecture because:
- **Duplication**: Two functions doing similar parsing work
- **Inconsistency**: Different interfaces for the same logical operation  
- **Maintenance Burden**: Changes need to be made in two places
- **Testing Issues**: Tests use different parsing than production
- **Confusion**: Which function should be used when?

## Current Usage

**In main.rs (production)**:
```rust
let command = match cli::parse_prompt_command_from_args(&args) {
    Ok(cmd) => cmd,
    Err(e) => {
        eprintln!("Invalid prompt command: {}", e);
        return EXIT_ERROR;
    }
};
```

**In tests**:
```rust
let parsed = parse_prompt_command(&matches).unwrap();
```

## Root Cause

The issue stems from main.rs trying to parse raw string args instead of using proper clap ArgMatches. This forces the creation of a second parsing function.

## Proposed Solution

**Use only `parse_prompt_command(matches: &ArgMatches)` everywhere**:

### 1. Fix main.rs to Use Proper Clap Parsing

**File**: `swissarmyhammer-cli/src/main.rs`

**Before**:
```rust
// Extract args from the prompt command  
let args: Vec<String> = matches
    .get_many::<String>("args")
    .map(|vals| vals.cloned().collect())
    .unwrap_or_default();

// Parse using the new CLI module
let command = match cli::parse_prompt_command_from_args(&args) {
```

**After**:
```rust
// Parse using proper clap matches
let command = match cli::parse_prompt_command(matches) {
```

### 2. Remove Duplicate Function

**File**: `swissarmyhammer-cli/src/commands/prompt/cli.rs`

- Remove `parse_prompt_command_from_args()` entirely
- Keep only `parse_prompt_command(matches: &ArgMatches)`
- Update all tests to use the single parsing function

### 3. Fix Command Definition in main.rs

The real issue is likely that main.rs isn't defining the prompt subcommands properly for clap to parse. We need to ensure that:
- Prompt subcommands are properly defined in the main CLI
- ArgMatches contains the parsed subcommand data
- No manual string argument parsing is needed

## Better Architecture

**Single parsing function**:
```rust
pub fn parse_prompt_command(matches: &ArgMatches) -> Result<PromptCommand, ParseError> {
    // Parse from proper clap matches, not raw strings
    // Used by both production and tests
}
```

**Main.rs uses standard clap flow**:
```rust
async fn handle_prompt_command(matches: &clap::ArgMatches, context: &CliContext) -> i32 {
    let command = match cli::parse_prompt_command(matches) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("Invalid prompt command: {}", e);
            return EXIT_ERROR;
        }
    };

    commands::prompt::handle_command_typed(command, context).await
}
```

## Why This is Better

1. **Single Source of Truth**: One parsing function used everywhere
2. **Proper clap Integration**: Uses ArgMatches as intended
3. **Consistent Behavior**: Tests and production use identical parsing
4. **Simpler Code**: No string manipulation or manual parsing
5. **Better Error Handling**: Clap provides better error messages

## Success Criteria

1. ✅ Only one prompt parsing function exists
2. ✅ Main.rs uses proper clap ArgMatches parsing  
3. ✅ All tests use the same parsing function as production
4. ✅ No manual string argument parsing
5. ✅ All existing functionality preserved
6. ✅ Clean, standard clap-based architecture

## Files Modified

- `swissarmyhammer-cli/src/commands/prompt/cli.rs` - Remove duplicate function
- `swissarmyhammer-cli/src/main.rs` - Use proper clap parsing
- Test files - Update to use single parsing function

---

**Priority**: Critical - Bad architecture foundation
**Estimated Effort**: Medium (architectural cleanup)  
**Dependencies**: None (fixing existing bad design)
**Blocks**: Clean prompt command implementation