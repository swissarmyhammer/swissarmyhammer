# PLAN_000013: Prompt Verification and Updates

**Refer to ./specification/plan.md**

## Goal

Verify and update the plan prompt in `builtin/prompts/plan.md` to properly accept and use the `plan_filename` parameter instead of hardcoded directory scanning.

## Background

The specification requires updating the plan prompt to work with a specific file parameter rather than scanning the entire `./specification` directory. The workflow has been updated but we need to ensure the prompt itself is properly configured.

## Requirements

1. Verify current state of `builtin/prompts/plan.md`
2. Update prompt to accept `plan_filename` argument if needed
3. Replace hardcoded `./specification` directory references with parameter
4. Update prompt instructions to work with specific file paths
5. Test parameter passing from workflow to prompt

## Implementation Details

### Expected Prompt Structure

The prompt should include:

```yaml
---
title: plan
description: Generate a step by step development plan from a specific specification file.
arguments:
  - name: plan_filename
    description: Path to the specific plan markdown file to process
    required: true
---
```

### Key Content Updates

1. **Process Section**: Update to read specific file instead of directory scanning
2. **References**: Replace hardcoded directory paths with `{{ plan_filename }}` parameter
3. **Instructions**: Update guidelines to reference the specific file parameter

## Acceptance Criteria

- [ ] Prompt accepts `plan_filename` argument
- [ ] All hardcoded `./specification` references replaced with parameter
- [ ] Process instructions updated for single file processing
- [ ] Parameter validation works correctly
- [ ] Prompt integrates properly with updated workflow

## Testing

- Verify prompt can be called with plan_filename parameter
- Test with various file paths (relative, absolute)
- Confirm parameter is properly used in prompt content
- Validate integration with workflow execution

## Notes

- Follow existing prompt argument patterns in the codebase
- Maintain backward compatibility with existing templates
- Ensure liquid template syntax is correctly implemented