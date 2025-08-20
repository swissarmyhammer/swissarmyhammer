# Validation Command Still Scanning Non-Prompt Files

## Problem
The validation command is still reporting many "Missing required field: title" errors, which suggests it's still scanning files that shouldn't be validated as prompts (like documentation files).

## Evidence
Running `./target/debug/swissarmyhammer validate --all --quiet` shows 26+ errors about missing titles, despite the validation.md issue claiming to be resolved.

## Root Cause
The validation scope fix from the previous validation.md issue may not be complete, or there may be additional files being scanned that shouldn't be.

## Expected Behavior
- Validation should only scan actual prompt files (*.md files in appropriate prompt directories)
- Documentation files, README files, and other non-prompt markdown files should be excluded
- Zero validation errors should be reported when all actual prompts are valid

## Acceptance Criteria
- [ ] `swissarmyhammer validate --all --quiet` returns exit code 0 with no errors
- [ ] Only actual prompt files are scanned (files in `.swissarmyhammer/`, `~/.swissarmyhammer/prompts/`, and builtin prompts)
- [ ] Documentation files (README.md, INSTALLATION.md, etc.) are excluded from validation
- [ ] Validation errors include file names to make debugging easier

## Implementation Notes
- Review the validation path filtering logic in validate.rs
- Ensure proper exclusion patterns for non-prompt directories and files
- Add file path to error messages for better debugging
- Consider adding a `--verbose` flag to show which files are being validated