I get

```
 sah doctor
2025-07-31T13:46:27.039985Z  INFO sah: Running diagnostics
🔨 SwissArmyHammer Doctor
Running diagnostics...

System Checks:
  ✓ swissarmyhammer in PATH - Found at: "/Users/wballard/.cargo/bin/swissarmyhammer"
  ✓ File permissions - Can read current directory: "/Users/wballard/github/swissarmyhammer"
  ✓ Workflow directory permissions: "workflows" - Directory has correct permissions: 755
  ✓ Workflow directory permissions: "runs" - Directory has correct permissions: 755

Configuration:
  ✓ Claude Code MCP configuration - swissarmyhammer is configured in Claude Code (found claude at: "/Users/wballard/.claude/local/claude")

Prompts:
  ✓ Built-in prompts - Built-in prompts are embedded in the binary
  ✓ User prompts directory - Found 0 prompts in "/Users/wballard/.swissarmyhammer/prompts"
  ✓ Local prompts directory - Local prompts directory not found (optional): ".swissarmyhammer/prompts"
    → Create directory: mkdir -p ".swissarmyhammer/prompts"
  ✓ YAML parsing - All prompt YAML front matter is valid

Workflows:
  ✓ User workflows directory - Found 0 workflows in "/Users/wballard/.swissarmyhammer/workflows"
  ✓ Local workflows directory - Local workflows directory not found (optional): ".swissarmyhammer/workflows"
    → Create directory: mkdir -p ".swissarmyhammer/workflows"
  ✓ Workflow run storage directory - Run storage directory exists: "/Users/wballard/.swissarmyhammer/runs"
  ✓ Workflow directory permissions: "workflows" - Directory has correct permissions: 755
  ✓ Workflow directory permissions: "runs" - Directory has correct permissions: 755
  ✓ Workflow parsing - All workflow files are readable
  ✓ Workflow run storage accessibility - Run storage is accessible and writable
  ✓ Workflow run storage space - Adequate disk space: 849806 MB
  ✓ Workflow name conflicts - No workflow name conflicts detected
  ✓ Workflow circular dependencies - Circular dependency checking requires workflow execution

Summary:
  19 checks passed, 1 warnings

```

-- but there is no obvious warning printed out!
## Proposed Solution

After examining the doctor command implementation, I found the issue:

The warning count in the summary shows "1 warnings" but no warning messages are visible to the user. Looking at the code, I can see that:

1. The `print_check` function (lines 284-312 in `doctor/mod.rs`) properly formats and displays individual check results including warnings
2. The `print_summary` function (lines 183-231) correctly counts and reports warning counts
3. However, the issue is that the warnings being generated have status `CheckStatus::Ok` instead of `CheckStatus::Warning`

Looking at the specific checks that could generate warnings:
- `check_prompt_directories` at lines 341-349 creates checks with `CheckStatus::Ok` for missing optional directories 
- `check_workflow_directories` at lines 477-479 creates checks with `CheckStatus::Ok` for missing optional directories

The issue is that these checks are marked as "Ok" even though they have fix suggestions, but the fix suggestions are being printed. However, since they're marked as Ok status, they shouldn't contribute to the warning count.

Let me investigate where the actual warning is coming from by examining the output more carefully and checking the check grouping logic.

## Implementation Steps

1. Run the doctor command and capture the actual checks being generated
2. Identify which specific check is generating the warning count without visible warning text
3. Fix the issue by either:
   - Properly displaying the warning message
   - Correcting the status of checks that should be warnings
   - Fixing the counting logic

4. Test the fix to ensure warnings are properly displayed
5. Add test cases to prevent regression
## Solution Implemented

### Root Cause Analysis
The issue was in the `group_checks_by_category` function in `swissarmyhammer-cli/src/doctor/mod.rs`. The "Binary Name" check that generates a warning when the binary name is not exactly "swissarmyhammer" was not being categorized into any section, so it was counted in the summary but not displayed to the user.

### Fix Applied
Updated the system checks categorization filter to include checks containing "Binary" and "Installation":

```rust
system_checks: self
  .checks
  .iter()
  .filter(|c| c.name.contains("PATH") || c.name.contains("permissions") || c.name.contains("Binary") || c.name.contains("Installation"))
  .collect(),
```

### Files Changed
- `swissarmyhammer-cli/src/doctor/mod.rs` - Added "Binary" and "Installation" to system checks filter
- `swissarmyhammer-cli/src/doctor/mod.rs` - Added regression test `test_warning_checks_are_categorized`

### Testing
1. ✅ Confirmed the warning is now visible when running `sah doctor`
2. ✅ All existing doctor tests pass
3. ✅ Added regression test to prevent this issue from happening again

### Results
Before fix:
```
Summary:
  19 checks passed, 1 warnings
```
(No warning visible to user)

After fix:
```
System Checks:
  ⚠ Binary Name - Unexpected binary name: sah
    → Consider renaming binary to 'swissarmyhammer'

Summary:
  19 checks passed, 1 warnings
```
(Warning properly displayed and categorized)

The issue is now completely resolved. The doctor command properly displays all warnings to the user.