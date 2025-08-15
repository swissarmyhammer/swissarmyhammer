when i run sah validate, i get 19 checks passed, 1 warnings -- but see no warning in the output

when i run sah validate, i get 19 checks passed, 1 warnings -- but see no warning in the output

## Proposed Solution

After investigating the validate command implementation in `swissarmyhammer-cli/src/validate.rs`, I found the issue lies in the `print_text_results` method. 

The current behavior is:
1. In quiet mode (`--quiet` flag), warnings are filtered out from display (lines 1047-1049) 
2. But the warning count is still shown in the summary regardless of quiet mode
3. This creates confusion when users see "1 warnings" in summary but no actual warning messages

The issue has two possible root causes:
1. **User is unknowingly in quiet mode**: If the `--quiet` flag is enabled (explicitly or through some default), warnings are suppressed but the count is still shown
2. **Inconsistent filtering logic**: There might be a bug where individual warnings are filtered out while the summary count remains

## Investigation Results

When I run `sah validate` normally (without `--quiet`), warnings are displayed correctly:
```
workflow:builtin:implement
  WARN [-] Circular dependency detected: are_issues_complete -> loop -> work
    💡 Ensure the workflow has proper exit conditions to avoid infinite loops
...
Summary:
  Files checked: 44
  Warnings: 3
⚠ Validation completed with warnings.
```

The `--quiet` flag is documented as "Only show errors, no warnings or info", so the behavior of hiding warnings in quiet mode is intentional.

## Root Cause Analysis

The most likely cause is that there's an inconsistency in how warnings are processed. The user should be seeing warning details unless they explicitly used `--quiet`. 

## Fix Strategy

I need to ensure that:
1. In normal mode (default), both warning details and summary are shown
2. In quiet mode (`--quiet`), neither warning details nor warning counts are shown in summary
3. The behavior is consistent between warning display and summary reporting

The fix will involve updating the `print_text_results` method to either:
- Always show warnings when they exist (unless explicit `--quiet`)
- Hide warning counts from summary when in quiet mode to maintain consistency