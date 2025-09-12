# Migrate Doctor Command to Follow Prompt Command Pattern

## Overview

Apply the architectural patterns we're establishing with the prompt command cleanup to the doctor command. This creates consistency across CLI commands and enables global argument support for doctor.

## Current Doctor State

**Good aspects**:
- Clean module structure with dedicated submodules (checks, types, utils)
- Business logic properly separated in submodules
- Help text from markdown file (`description.md`)
- Comprehensive functionality with good test coverage

**Needs updating**:
- Takes `TemplateContext` instead of `CliContext`
- No support for global `--verbose` and `--format` arguments
- Manual output formatting instead of using display abstractions

## Pattern to Apply

Based on prompt command refactoring, apply these patterns:

### 1. CliContext Integration

**Current signature**:
```rust
pub async fn handle_command(_template_context: &TemplateContext) -> i32
```

**Target signature**:
```rust
pub async fn handle_command(cli_context: &CliContext) -> i32
```

### 2. Global Argument Support

**Enable for doctor**:
```bash
sah --verbose doctor                    # Verbose diagnostic output
sah --format=json doctor               # JSON output for scripting
sah --format=yaml doctor               # YAML output for scripting
```

### 3. Display Object Pattern

**Create display objects**:
```rust
#[derive(Tabled, Serialize)]
pub struct CheckResult {
    pub name: String,
    pub status: String,
    pub message: String,
}

#[derive(Tabled, Serialize)]
pub struct VerboseCheckResult {
    pub name: String,
    pub status: String,
    pub message: String,
    pub fix: Option<String>,
    pub category: String,
}
```

### 4. CliContext Display Integration

**Use CliContext for output**:
```rust
// Instead of manual println! formatting
let check_results: Vec<CheckResult> = doctor.checks
    .iter()
    .map(|check| check.into())
    .collect();

cli_context.display(check_results)?;
```

## Implementation Steps

### 1. Update CliContext for Doctor

**File**: `swissarmyhammer-cli/src/context.rs`

```rust
impl CliContext {
    // Add any doctor-specific context methods if needed
    // (Doctor is likely self-contained and won't need special context methods)
}
```

### 2. Create Display Objects

**File**: `swissarmyhammer-cli/src/commands/doctor/display.rs`

```rust
use tabled::Tabled;
use serde::Serialize;
use super::types::{Check, CheckStatus};

#[derive(Tabled, Serialize)]
pub struct CheckResult {
    #[tabled(rename = "Status")]
    pub status: String,
    #[tabled(rename = "Check")]
    pub name: String,
    #[tabled(rename = "Result")]
    pub message: String,
}

#[derive(Tabled, Serialize)]
pub struct VerboseCheckResult {
    #[tabled(rename = "Status")]
    pub status: String,
    #[tabled(rename = "Check")]
    pub name: String,
    #[tabled(rename = "Result")]
    pub message: String,
    #[tabled(rename = "Fix")]
    pub fix: String,
    #[tabled(rename = "Category")]
    pub category: String,
}

impl From<&Check> for CheckResult {
    fn from(check: &Check) -> Self {
        Self {
            status: format_check_status(&check.status),
            name: check.name.clone(),
            message: check.message.clone(),
        }
    }
}

impl From<&Check> for VerboseCheckResult {
    fn from(check: &Check) -> Self {
        Self {
            status: format_check_status(&check.status),
            name: check.name.clone(),
            message: check.message.clone(),
            fix: check.fix.clone().unwrap_or_else(|| "No fix available".to_string()),
            category: categorize_check(check),
        }
    }
}

fn format_check_status(status: &CheckStatus) -> String {
    match status {
        CheckStatus::Ok => "✓".to_string(),
        CheckStatus::Warning => "⚠".to_string(),
        CheckStatus::Error => "✗".to_string(),
    }
}

fn categorize_check(check: &Check) -> String {
    // Logic to categorize checks (System, Config, Prompt, Workflow)
    if check.name.contains("Binary") || check.name.contains("PATH") {
        "System".to_string()
    } else if check.name.contains("Claude") || check.name.contains("Config") {
        "Config".to_string()
    } else if check.name.contains("Prompt") {
        "Prompt".to_string()
    } else if check.name.contains("Workflow") {
        "Workflow".to_string()
    } else {
        "Other".to_string()
    }
}
```

### 3. Update Doctor to Use CliContext

**File**: `swissarmyhammer-cli/src/commands/doctor/mod.rs`

```rust
use crate::context::CliContext;
use super::display::{CheckResult, VerboseCheckResult};

/// Handle the doctor command
pub async fn handle_command(cli_context: &CliContext) -> i32 {
    let mut doctor = Doctor::new();

    match run_doctor_diagnostics(&mut doctor, cli_context).await {
        Ok(exit_code) => exit_code,
        Err(e) => {
            eprintln!("Doctor command failed: {}", e);
            EXIT_ERROR
        }
    }
}

async fn run_doctor_diagnostics(doctor: &mut Doctor, cli_context: &CliContext) -> Result<i32> {
    // Run all diagnostics (existing logic)
    doctor.run_diagnostics_with_options()?;

    // Format and display results using CliContext
    if cli_context.verbose {
        let verbose_results: Vec<VerboseCheckResult> = doctor.checks
            .iter()
            .map(|check| check.into())
            .collect();
        cli_context.display(verbose_results)?;
    } else {
        let results: Vec<CheckResult> = doctor.checks
            .iter()
            .map(|check| check.into())
            .collect();
        cli_context.display(results)?;
    }

    Ok(doctor.get_exit_code())
}
```

### 4. Update Main.rs Integration

**File**: `swissarmyhammer-cli/src/main.rs`

```rust
// Change from:
commands::doctor::handle_command(template_context).await

// To:
commands::doctor::handle_command(&cli_context).await
```

## Benefits

### For Users
- **Global arguments work**: `sah --verbose doctor`, `sah --format=json doctor`
- **Consistent output**: Same table/json/yaml formatting as other commands
- **Better scripting**: JSON/YAML output for automation

### For Developers  
- **Consistent patterns**: Doctor follows same architecture as prompt
- **Reusable display**: Same display abstractions across commands
- **Easier maintenance**: Standard CliContext integration

### For Architecture
- **Pattern validation**: Proves the CliContext pattern works for different command types
- **Consistency**: All commands follow same architectural patterns
- **Foundation**: Sets up pattern for migrating remaining commands

## Success Criteria

1. ✅ `sah doctor` works exactly as before
2. ✅ `sah --verbose doctor` shows detailed output
3. ✅ `sah --format=json doctor` outputs JSON
4. ✅ `sah --format=yaml doctor` outputs YAML  
5. ✅ All existing doctor functionality preserved
6. ✅ Clean table output using tabled
7. ✅ Consistent error handling and exit codes
8. ✅ No duplicate code with prompt command patterns

## Files Created

- `swissarmyhammer-cli/src/commands/doctor/display.rs` - Display objects

## Files Modified  

- `swissarmyhammer-cli/src/commands/doctor/mod.rs` - CliContext integration
- `swissarmyhammer-cli/src/main.rs` - Pass CliContext instead of TemplateContext

---

**Priority**: Medium - Validates pattern established by prompt cleanup
**Estimated Effort**: Medium
**Dependencies**: cli_prompt_000001_add_global_format_argument (for CliContext)
**Benefits**: Proves CliContext pattern works for multiple command types

## Proposed Solution

After analyzing the current doctor command implementation, here's my step-by-step approach to migrate it to follow the prompt command pattern:

### Analysis Summary
- **Current**: Uses `TemplateContext`, manual formatting with `colored` crate, custom display logic
- **Target**: Use `CliContext`, standardized display objects with `Tabled` + `Serialize`, leverage `CliContext::display()` method

### Implementation Plan

#### 1. Create Display Objects (`display.rs`)
- `CheckResult`: Basic check info (status symbol, name, message)  
- `VerboseCheckResult`: Extended info (+ fix suggestion, category)
- Both derive `Tabled` and `Serialize` for consistent output formatting
- Implement `From<&Check>` conversions
- Add helper functions for status formatting and categorization

#### 2. Update Doctor Module
- Change signature: `handle_command(cli_context: &CliContext)` 
- Replace manual `println!` with `cli_context.display()`
- Use `cli_context.verbose` flag to choose display format
- Preserve all existing diagnostic logic and exit code behavior

#### 3. Update Main.rs Integration
- Pass `&cli_context` instead of `template_context` to doctor command
- This enables global `--verbose` and `--format` arguments automatically

### Benefits
- **Consistency**: Same architectural pattern as prompt command
- **Global Args**: `sah --verbose doctor` and `sah --format=json doctor` work
- **Maintainability**: Reusable display abstractions
- **User Experience**: Clean table/JSON/YAML output options

### Files to Create/Modify
- **Create**: `swissarmyhammer-cli/src/commands/doctor/display.rs`
- **Modify**: `swissarmyhammer-cli/src/commands/doctor/mod.rs` 
- **Modify**: `swissarmyhammer-cli/src/main.rs` (one line change)

This approach preserves all existing functionality while adding the architectural consistency and global argument support.
## Implementation Summary

Successfully migrated the doctor command to follow the prompt command pattern! All implementation goals achieved:

### ✅ Completed Tasks

1. **Created `display.rs` module** - Added `CheckResult` and `VerboseCheckResult` structs with `Tabled` + `Serialize` support
2. **Updated doctor module** - Changed to use `CliContext` instead of `TemplateContext` 
3. **Updated main.rs integration** - One-line change to pass `&context` instead of `&template_context`
4. **Preserved all functionality** - All 51 existing tests still pass

### ✅ Functionality Verification

**Basic command works exactly as before:**
```bash
sah doctor
```

**Global verbose flag now works:**
```bash
sah --verbose doctor
```
Shows additional Fix and Category columns in table output.

**Global format flags now work:**
```bash
sah --format=json doctor    # JSON output
sah --format=yaml doctor    # YAML output  
```

### 📋 Success Criteria Met

- ✅ `sah doctor` works exactly as before
- ✅ `sah --verbose doctor` shows detailed output with Fix and Category columns
- ✅ `sah --format=json doctor` outputs clean JSON
- ✅ `sah --format=yaml doctor` outputs clean YAML  
- ✅ All existing doctor functionality preserved
- ✅ Clean table output using tabled
- ✅ Consistent error handling and exit codes maintained
- ✅ Follows same architectural pattern as prompt command

### 📁 Files Modified

**Created:**
- `swissarmyhammer-cli/src/commands/doctor/display.rs` - Display objects with full test coverage

**Modified:**
- `swissarmyhammer-cli/src/commands/doctor/mod.rs` - Added CliContext integration and display module
- `swissarmyhammer-cli/src/main.rs` - Updated to pass CliContext to doctor command

### 🎯 Architecture Benefits Achieved

- **Global Arguments**: Doctor command now supports `--verbose` and `--format` flags
- **Consistency**: Same display patterns as prompt command 
- **Maintainability**: Reusable display abstractions
- **User Experience**: Multiple output formats for scripting
- **Pattern Validation**: Proves CliContext works for different command types

The migration is complete and validates the architectural pattern for future command migrations!