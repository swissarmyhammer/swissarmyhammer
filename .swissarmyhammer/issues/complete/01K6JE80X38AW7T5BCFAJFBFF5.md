# Remove consistent-naming rule (redundant with standard linters)

## Problem
The `consistent-naming` rule is redundant with standard linters like clippy, eslint, pylint, etc. These tools already enforce naming conventions effectively and are widely adopted.

## Rationale
- Standard linters (clippy for Rust, eslint for JS/TS, pylint for Python) already check naming conventions
- These tools are faster, more mature, and better integrated into development workflows
- Duplicating this functionality adds maintenance burden without adding value
- Users expect naming checks from their language-specific linters, not from a general-purpose AI linter

## Action
Remove the `consistent-naming` rule from the builtin rules.

## Impact
- Reduces maintenance burden
- Avoids confusion about which tool should enforce naming
- Users should continue using language-specific linters for naming conventions



## Proposed Solution

The consistent-naming rule is located at `builtin/rules/code-quality/consistent-naming.md`. To remove it, I will:

1. Delete the rule file: `builtin/rules/code-quality/consistent-naming.md`
2. Verify that the build system (build.rs) will automatically exclude it from the generated builtin_rules.rs
3. Run tests to ensure no references remain and the system works without it

The build.rs script automatically scans the builtin/rules directory and generates the embedded rules list, so simply removing the file should be sufficient.

## Implementation Notes



### Deletion Completed
- Deleted `/Users/wballard/github/sah/builtin/rules/code-quality/consistent-naming.md`
- Verified no code references remain (only documentation references in ideas/rules.md)
- Build succeeded: `cargo build` completed in 8.28s
- All tests passed: 3225 tests run, 3225 passed, 1 skipped (57.177s)

The build.rs script automatically regenerates builtin_rules.rs during compilation, so the rule has been successfully removed from the embedded builtin rules without requiring any code changes.
