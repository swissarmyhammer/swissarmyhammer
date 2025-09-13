# Migrate Validate Command to Use Consistent Output Pattern

## Overview

Apply the same output consistency patterns established with doctor and prompt commands to the validate command. Currently validate has mixed output formatting that should be converted to structured table format.

## Current Validate Output Issues

The validate command has inconsistent output formatting:

```
.system                                    # File-based grouping
  prompt:.system                          # Indented items
  WARN [-] Template uses variables...      # Manual formatting
    💡 Define parameters for...            # Manual fix suggestions

Summary:                                   # Manual summary
  Files checked: 76                       # Manual stats
  Warnings: 1                             # Manual counts

⚠ Validation completed with warnings.     # Manual status message
```

**Problems**:
- Mixed manual formatting instead of structured table
- No support for global `--verbose` and `--format` arguments
- Manual output handling instead of using CliContext
- Takes individual parameters instead of CliContext

## Target Output Format

**Standard table format**:
```
┌────────┬─────────────────────┬────────────────────────────────────┐
│ Status │ File                │ Result                             │
├────────┼─────────────────────┼────────────────────────────────────┤
│ ✓      │ .system             │ Valid                              │
│ ⚠      │ prompt:.system      │ Template uses undefined variables  │
│ ✓      │ say-hello.md        │ Valid                              │
└────────┴─────────────────────┴────────────────────────────────────┘
```

**Verbose table format** (`sah --verbose validate`):
```
┌────────┬─────────────────────┬────────────────────────────────────┬─────────────────────────┐
│ Status │ File                │ Result                             │ Fix                     │
├────────┼─────────────────────┼────────────────────────────────────┼─────────────────────────┤
│ ✓      │ .system             │ Valid                              │                         │
│ ⚠      │ prompt:.system      │ Template uses undefined variables  │ Define parameters       │
│ ✓      │ say-hello.md        │ Valid                              │                         │
└────────┴─────────────────────┴────────────────────────────────────┴─────────────────────────┘
```

**JSON format** (`sah --format=json validate`):
```json
[
  {"status": "✓", "file": ".system", "result": "Valid"},
  {"status": "⚠", "file": "prompt:.system", "result": "Template uses undefined variables", "fix": "Define parameters"}
]
```

## Implementation Steps

### 1. Update Command Signature

**File**: `swissarmyhammer-cli/src/commands/validate/mod.rs`

**Current signature**:
```rust
pub async fn handle_command(
    quiet: bool,
    format: OutputFormat,
    workflow_dirs: Vec<String>,
    validate_tools: bool,
    _template_context: &TemplateContext,
) -> i32
```

**Target signature**:
```rust
pub async fn handle_command(
    workflow_dirs: Vec<String>,
    validate_tools: bool,
    cli_context: &CliContext,
) -> i32
```

Note: `quiet` and `format` come from `cli_context` global args.

### 2. Create Display Objects

**File**: `swissarmyhammer-cli/src/commands/validate/display.rs`

```rust
use tabled::Tabled;
use serde::Serialize;

#[derive(Tabled, Serialize)]
pub struct ValidationResult {
    #[tabled(rename = "Status")]
    pub status: String,
    
    #[tabled(rename = "File")]
    pub file: String,
    
    #[tabled(rename = "Result")]
    pub result: String,
}

#[derive(Tabled, Serialize)]
pub struct VerboseValidationResult {
    #[tabled(rename = "Status")]
    pub status: String,
    
    #[tabled(rename = "File")]
    pub file: String,
    
    #[tabled(rename = "Result")]
    pub result: String,
    
    #[tabled(rename = "Fix")]
    pub fix: String,
    
    #[tabled(rename = "Type")]
    pub file_type: String,
}

impl From<&ValidationIssue> for ValidationResult {
    fn from(issue: &ValidationIssue) -> Self {
        Self {
            status: format_validation_status(&issue.level),
            file: issue.file.clone(),
            result: issue.message.clone(),
        }
    }
}

impl From<&ValidationIssue> for VerboseValidationResult {
    fn from(issue: &ValidationIssue) -> Self {
        Self {
            status: format_validation_status(&issue.level),
            file: issue.file.clone(),
            result: issue.message.clone(),
            fix: issue.fix.clone().unwrap_or_else(|| "No fix available".to_string()),
            file_type: determine_file_type(&issue.file),
        }
    }
}

fn format_validation_status(level: &ValidationLevel) -> String {
    match level {
        ValidationLevel::Ok => "✓".to_string(),
        ValidationLevel::Warning => "⚠".to_string(),
        ValidationLevel::Error => "✗".to_string(),
    }
}

fn determine_file_type(file: &str) -> String {
    if file.ends_with(".md") {
        "Prompt".to_string()
    } else if file.ends_with(".yaml") || file.ends_with(".yml") {
        "Workflow".to_string()
    } else {
        "Other".to_string()
    }
}
```

### 3. Update Validation Logic

**File**: `swissarmyhammer-cli/src/validate.rs`

Update `run_validate_command_with_dirs` to:
- Return structured validation results instead of printing directly
- Collect all validation issues into a structured format
- Remove manual println! calls and summary formatting

**New approach**:
```rust
pub struct ValidationResults {
    pub issues: Vec<ValidationIssue>,
    pub files_checked: usize,
    pub summary: ValidationSummary,
}

pub struct ValidationIssue {
    pub file: String,
    pub level: ValidationLevel,
    pub message: String,
    pub fix: Option<String>,
}

pub enum ValidationLevel {
    Ok,
    Warning, 
    Error,
}

pub struct ValidationSummary {
    pub files_checked: usize,
    pub warnings: usize,
    pub errors: usize,
}
```

### 4. Update Main Integration

**File**: `swissarmyhammer-cli/src/main.rs`

```rust
// Change from multiple individual parameters to CliContext
commands::validate::handle_command(workflow_dirs, validate_tools, &cli_context).await
```

### 5. Add CliContext Display Integration

**File**: `swissarmyhammer-cli/src/commands/validate/mod.rs`

```rust
pub async fn handle_command(
    workflow_dirs: Vec<String>,
    validate_tools: bool,
    cli_context: &CliContext,
) -> i32 {
    // Run validation and collect structured results
    let results = validate::run_validation_structured(workflow_dirs, validate_tools).await?;
    
    // Convert to display format
    if cli_context.verbose {
        let verbose_results: Vec<VerboseValidationResult> = results.issues
            .iter()
            .map(|issue| issue.into())
            .collect();
        cli_context.display(verbose_results)?;
    } else {
        let display_results: Vec<ValidationResult> = results.issues
            .iter()
            .map(|issue| issue.into())
            .collect();
        cli_context.display(display_results)?;
    }
    
    // Return appropriate exit code based on validation results
    if results.summary.errors > 0 {
        EXIT_ERROR
    } else if results.summary.warnings > 0 {
        EXIT_WARNING  
    } else {
        EXIT_SUCCESS
    }
}
```

## Benefits

### For Users
- **Global arguments work**: `sah --verbose validate`, `sah --format=json validate`
- **Consistent output**: Same table formatting as doctor and prompt commands
- **Better scripting**: JSON/YAML output for CI/CD automation
- **Structured data**: Easy to parse validation results programmatically

### For Developers
- **Consistent patterns**: Validate follows same architecture as other commands
- **Reusable display**: Same display abstractions across all commands
- **Easier maintenance**: Standard CliContext integration
- **Better testing**: Structured results easier to test

### For Architecture
- **Pattern completion**: Third command using CliContext pattern
- **Consistency**: All major commands follow same architectural patterns
- **Proven approach**: Validates that the pattern works across different command types

## Success Criteria

1. ✅ `sah validate` works exactly as before functionally
2. ✅ `sah --verbose validate` shows detailed validation information
3. ✅ `sah --format=json validate` outputs structured JSON
4. ✅ `sah --format=yaml validate` outputs structured YAML
5. ✅ All validation logic preserved (prompts, workflows, tools)
6. ✅ Clean table output using tabled
7. ✅ Consistent error handling and exit codes
8. ✅ No scattered println! calls - all output through CliContext

## Files Created

- `swissarmyhammer-cli/src/commands/validate/display.rs` - Display objects

## Files Modified

- `swissarmyhammer-cli/src/commands/validate/mod.rs` - CliContext integration
- `swissarmyhammer-cli/src/validate.rs` - Return structured results
- `swissarmyhammer-cli/src/main.rs` - Pass CliContext instead of individual parameters

---

**Priority**: Medium - Completes the CliContext pattern across major commands
**Estimated Effort**: Medium
**Dependencies**: cli_prompt_000001_add_global_format_argument (for CliContext)
**Benefits**: Proves CliContext pattern is universal across command types

## Proposed Solution

Based on my analysis of the current code and the CliContext pattern used by doctor and prompt commands, here's my implementation approach:

### Key Changes Required:

1. **Update Command Signature**: Change `handle_command` to take `CliContext` instead of individual parameters (`quiet`, `format`, etc.)

2. **Create Display Objects**: Create `display.rs` with `ValidationResult` and `VerboseValidationResult` structs that implement `Tabled` and `Serialize` traits

3. **Refactor Core Validation**: Modify `validate.rs` to return structured results instead of directly printing output

4. **Integrate CliContext Display**: Use `cli_context.display()` method to output results in the requested format

### Implementation Strategy:

The validate command already has good structured data (`ValidationResult`, `ValidationIssue`) internally. The main work is:
- Creating display wrapper structs that map from internal types to display format  
- Removing direct print statements and returning structured data
- Using CliContext's display method for consistent formatting

This follows the exact same pattern as the doctor command:
1. Run validation logic to collect results
2. Convert internal results to display objects
3. Use `cli_context.display()` for output formatting
4. Return appropriate exit codes

The migration maintains all existing functionality while adding support for global `--verbose` and `--format` arguments.

## Progress Update

### Completed:
✅ Created display.rs with ValidationResult and VerboseValidationResult structs
✅ Updated validate command signature to use CliContext pattern
✅ Added structured validation function to validate.rs
✅ Updated main.rs integration to pass CliContext

### Current Issue:
❌ The validate command is still using the old output path despite code changes

**Problem**: Even after updating the command to use CliContext, the output still shows the old text-based format. Debug messages I added don't appear, suggesting the new code path isn't being called at all.

**Investigation needed**: 
- Check if there are compilation errors preventing the new code from being used
- Verify the function call chain is correct
- Ensure no other code path is still calling the old functions