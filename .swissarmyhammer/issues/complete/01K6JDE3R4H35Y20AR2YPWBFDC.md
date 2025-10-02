# Fix `rule validate` command panic

## Problem
Running `cargo run -- rule validate --rule test-rule` panics with:
```
thread 'main' panicked at swissarmyhammer-cli/src/commands/rule/cli.rs:92:40:
Mismatch between definition and access of `rule_name`. Unknown argument or group id.
```

## Root Cause
The issue is a mismatch between the CLI argument definition and the parser:

### CLI Definition (dynamic_cli.rs:1250)
```rust
.arg(
    Arg::new("rule")  // ← argument is named "rule"
        .long("rule")
        .help("Validate specific rule by name")
        .value_name("NAME"),
)
```

### Parser (cli.rs:92)
```rust
let validate_cmd = ValidateCommand {
    rule_name: sub_matches.get_one::<String>("rule_name").cloned(),  // ← tries to access "rule_name"
    file: sub_matches.get_one::<String>("file").cloned(),
};
```

## Proposed Solution

### Step 1: Fix the parser to match CLI definition
Change line 92 in `swissarmyhammer-cli/src/commands/rule/cli.rs` from:
```rust
rule_name: sub_matches.get_one::<String>("rule_name").cloned(),
```
to:
```rust
rule_name: sub_matches.get_one::<String>("rule").cloned(),
```

### Step 2: Update tests to match actual CLI
The tests currently define `rule_name` as a positional argument, but the actual CLI uses `--rule` as a long flag. Update tests to use the correct definition.

## Testing Plan
1. Fix the parser to use "rule" instead of "rule_name"
2. Update test command definitions to match actual CLI
3. Run `cargo nextest run` to verify all tests pass
4. Manually test: `cargo run -- rule validate --rule test-rule`
5. Verify no panic occurs



## Implementation

### Changes Made

1. **Fixed parser in cli.rs:92**
   - Changed `sub_matches.get_one::<String>("rule_name")` to `sub_matches.get_one::<String>("rule")`
   - This matches the CLI definition in dynamic_cli.rs:1250 where the argument is named "rule"

2. **Updated tests to match actual CLI**
   - Modified `test_parse_validate_command_with_rule_name()` to use `--rule` flag instead of positional argument
   - Modified `test_parse_validate_command_with_file()` to use `--rule` and `--file` flags
   - Changed test command definitions from `.arg(Arg::new("rule_name").index(1))` to `.arg(Arg::new("rule").long("rule").value_name("NAME"))`

### Testing Results

✅ All unit tests pass (12 tests in rule CLI module)
✅ Command no longer panics: `cargo run -- rule validate --rule security/no-eval`
✅ Validates successfully: `✓ 1 valid rule(s)`
✅ Handles missing rules gracefully: `Rule 'test-rule' not found`
✅ Validates all rules: `cargo run -- rule validate` → `✓ 11 valid rule(s)`

### Root Cause Confirmed

The panic was caused by clap's argument validation. When the parser tried to access an argument named "rule_name" but the CLI only defined an argument named "rule", clap detected this mismatch and panicked with the error message:

```
Mismatch between definition and access of `rule_name`. Unknown argument or group id.
```

The fix ensures the parser accesses the correct argument name that matches the CLI definition.
