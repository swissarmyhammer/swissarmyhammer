# Step 1: Rename TDD Workflow File

Refer to /Users/wballard/github/sah/ideas/test.md

## Objective
Rename the existing TDD workflow file from `builtin/workflows/tdd.md` to `builtin/workflows/test.md` and update its metadata.

## Task Details

### File Operations
- **Source**: `builtin/workflows/tdd.md`
- **Target**: `builtin/workflows/test.md`
- Use `git mv` to preserve history

### Content Updates  
- Update workflow title from "TDD" to "Test"
- Update description from "Autonomously run a TDD loop until all tests pass" to "Autonomously run a test loop until all tests pass"
- Keep all states, actions, and mermaid diagrams unchanged
- Preserve all functionality and workflow logic

## Expected Changes
```yaml
---
title: Test  # Changed from "TDD"
description: Autonomously run a test loop until all tests pass  # Updated
tags:
  - auto
---
# Rest of file unchanged
```

## Validation
- Verify file exists at new location
- Verify old location no longer exists  
- Confirm git tracks the rename operation
- Check that workflow content is preserved
- Test workflow can be loaded: `sah flow list | grep -i test`

## Size Estimate
~10 lines of changes (metadata only)

## Dependencies
None - this is an independent file operation.