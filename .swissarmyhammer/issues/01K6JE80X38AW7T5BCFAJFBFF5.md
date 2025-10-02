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
