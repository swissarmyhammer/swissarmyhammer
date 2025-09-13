# Migrate Doctor Command to Follow Prompt Command Pattern

## Overview

Apply the architectural patterns from prompt command cleanup to doctor command, with specific focus on getting ALL output into consistent table format instead of the current mixed output approach.

## Current Output Problems

The doctor command has inconsistent output with multiple styles:

```
🔨 SwissArmyHammer Doctor              # Direct println!
Running diagnostics...                 # Direct println!

✅ Git repository detected at: /path   # Direct println!
✅ .swissarmyhammer directory found    # Direct println!  
  ✅ Directory is accessible           # Indented println!
  ✅ Directory is writable            # Indented println!
  ✅ memos/ (8 items)                 # Indented println!

┌────────┬─────────────────────────────┬─────────┐  # Proper table
│ Status │ Check                       │ Result  │  # (only some output)
├────────┼─────────────────────────────┼─────────┤
│ ✓      │ Installation Method         │ ...     │
└────────┴─────────────────────────────┴─────────┘
```

**Problem**: Mixed output styles, inconsistent formatting, some output outside the table.

## Goal: All Output in Table

**Target**: Everything should be in the structured table format, with different verbosity levels controlling detail.

**Standard output**:
```
┌────────┬─────────────────────────────┬─────────┐
│ Status │ Check                       │ Result  │
├────────┼─────────────────────────────┼─────────┤
│ ✓      │ Git Repository              │ Found   │
│ ✓      │ SwissArmyHammer Directory   │ Found   │
│ ✓      │ Installation Method         │ Dev     │
│ ✓      │ Binary Permissions          │ 755     │
└────────┴─────────────────────────────┴─────────┘
```

**Verbose output**:
```
┌────────┬─────────────────────────────┬─────────────────────────────────────┬─────────────────┐
│ Status │ Check                       │ Result                              │ Details         │
├────────┼─────────────────────────────┼─────────────────────────────────────┼─────────────────┤
│ ✓      │ Git Repository              │ Found                               │ /path/to/repo   │
│ ✓      │ SwissArmyHammer Directory   │ Found                               │ /path/.sah      │
│ ✓      │ Memos Storage               │ 8 items                             │ /path/memos     │
│ ✓      │ Installation Method         │ Development build                   │ v0.1.0 debug    │
└────────┴─────────────────────────────┴─────────────────────────────────────┴─────────────────┘
```

## Implementation Steps

### 1. Create Comprehensive Display Objects

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
    #[tabled(rename = "Details")]
    pub details: String,
    #[tabled(rename = "Fix")]
    pub fix: String,
}
```

### 2. Convert All Output to Table Format

**Current scattered output to convert**:
- Git repository detection → "Git Repository" check
- SwissArmyHammer directory analysis → "SwissArmyHammer Directory" check
- Individual directory items → Separate checks for memos/, todo/, runs/, etc.
- File permission checks → "File Permissions" check
- All diagnostic messages → Structured check results

**Updated Doctor Logic**:
```rust
impl Doctor {
    pub fn run_diagnostics_with_options(&mut self) -> Result<ExitCode> {
        // Convert ALL diagnostic output to Check objects
        self.check_git_repository()?;
        self.check_swissarmyhammer_directory()?;
        self.check_directory_contents()?;
        self.run_system_checks()?;
        self.run_configuration_checks()?;
        self.run_prompt_checks()?;
        self.run_workflow_checks()?;
        
        // NO direct println! calls - everything goes into self.checks
        Ok(self.get_exit_code())
    }

    fn check_git_repository(&mut self) -> Result<()> {
        // Instead of: println!("✅ Git repository detected at: {}", path);
        // Do: self.checks.push(Check { name: "Git Repository", status: Ok, message: "Found", ... });
    }

    fn check_swissarmyhammer_directory(&mut self) -> Result<()> {
        // Convert all the directory analysis println! calls to Check objects
    }
}
```

### 3. Update Output Handling

**File**: `swissarmyhammer-cli/src/commands/doctor/mod.rs`

```rust
pub async fn handle_command(cli_context: &CliContext) -> i32 {
    let mut doctor = Doctor::new();

    match doctor.run_diagnostics_with_options() {
        Ok(exit_code) => {
            // ALL output goes through CliContext display
            if cli_context.verbose {
                let verbose_results: Vec<VerboseCheckResult> = doctor.checks
                    .iter()
                    .map(|check| check.into())
                    .collect();
                if let Err(e) = cli_context.display(verbose_results) {
                    eprintln!("Display error: {}", e);
                    return EXIT_ERROR;
                }
            } else {
                let results: Vec<CheckResult> = doctor.checks
                    .iter()
                    .map(|check| check.into())
                    .collect();
                if let Err(e) = cli_context.display(results) {
                    eprintln!("Display error: {}", e);
                    return EXIT_ERROR;
                }
            }
            
            exit_code.into()
        }
        Err(e) => {
            eprintln!("Doctor command failed: {}", e);
            EXIT_ERROR
        }
    }
}
```

### 4. Remove All Direct println! Calls

**Target**: Remove ALL direct output from doctor logic:
- No more `println!("🔨 SwissArmyHammer Doctor")`
- No more `println!("Running diagnostics...")`
- No more `println!("✅ Git repository detected...")`
- No more manual formatting or colored output

**Everything becomes Check objects** that get displayed via CliContext table formatting.

## Expected Result

**Standard format** (`sah doctor`):
```
┌────────┬─────────────────────────────┬─────────────────────────┐
│ Status │ Check                       │ Result                  │
├────────┼─────────────────────────────┼─────────────────────────┤
│ ✓      │ Git Repository              │ Found                   │
│ ✓      │ SwissArmyHammer Directory   │ Found                   │
│ ✓      │ Memos Storage               │ 8 items                 │
│ ⚠      │ Runs Directory              │ Will be created         │
│ ✓      │ Installation Method         │ Development build       │
│ ✓      │ Binary Permissions          │ 755                     │
│ ✓      │ Claude Code Integration     │ Configured              │
└────────┴─────────────────────────────┴─────────────────────────┘
```

**JSON format** (`sah --format=json doctor`):
```json
[
  {"status": "✓", "name": "Git Repository", "message": "Found"},
  {"status": "✓", "name": "SwissArmyHammer Directory", "message": "Found"},
  {"status": "⚠", "name": "Runs Directory", "message": "Will be created"}
]
```

**Verbose format** (`sah --verbose doctor`):
```
┌────────┬─────────────────────────────┬─────────────────────────┬─────────────────────┬──────────────┐
│ Status │ Check                       │ Result                  │ Details             │ Fix          │
├────────┼─────────────────────────────┼─────────────────────────┼─────────────────────┼──────────────┤
│ ✓      │ Git Repository              │ Found                   │ /path/to/repo       │              │
│ ✓      │ SwissArmyHammer Directory   │ Found                   │ /path/.sah          │              │
│ ⚠      │ Runs Directory              │ Will be created         │ Not critical        │ Run workflow │
└────────┴─────────────────────────────┴─────────────────────────┴─────────────────────┴──────────────┘
```

## Success Criteria

1. ✅ NO direct println! calls in doctor logic
2. ✅ ALL output goes through CliContext.display()
3. ✅ Consistent table formatting across all output modes
4. ✅ Global `--verbose` and `--format` arguments work
5. ✅ JSON/YAML output contains all diagnostic information
6. ✅ Same information available, just better formatted
7. ✅ All existing functionality preserved

## Files Created

- `swissarmyhammer-cli/src/commands/doctor/display.rs` - Display objects

## Files Modified  

- `swissarmyhammer-cli/src/commands/doctor/mod.rs` - CliContext integration, remove println!
- `swissarmyhammer-cli/src/commands/doctor/checks.rs` - Convert output to Check objects
- `swissarmyhammer-cli/src/main.rs` - Pass CliContext instead of TemplateContext

---

**Priority**: Medium - Improves output consistency
**Estimated Effort**: Medium (convert output calls to Check objects)
**Dependencies**: cli_prompt_000001_add_global_format_argument