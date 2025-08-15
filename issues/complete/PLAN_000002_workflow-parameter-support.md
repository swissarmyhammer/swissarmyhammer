# PLAN_000002: Workflow Parameter Support

**Refer to ./specification/plan.md**

## Goal

Update the existing plan workflow in `builtin/workflows/plan.md` to accept a `plan_filename` parameter and pass it to the plan prompt execution.

## Background

The current plan workflow is hardcoded and doesn't accept parameters. We need to modify it to accept a `plan_filename` parameter that can be passed from the CLI command to specify which file to plan.

## Requirements

1. Add parameter definition section to workflow YAML front matter
2. Update workflow actions to use the parameter
3. Modify log messages to include the filename
4. Ensure parameter is passed to prompt execution
5. Maintain existing workflow structure and state machine

## Implementation Details

### Current Workflow Structure

The existing workflow in `builtin/workflows/plan.md`:

```yaml
---
title: Plan
description: Create a plan from a specification
tags:
  - auto
---
```

### Updated Workflow Structure

Replace with:

```yaml
---
title: Plan
description: Create a plan from a specification
tags:
  - auto
---

## Parameters

- plan_filename: The path to the specific plan file to process

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> plan
    plan --> done
    done --> [*]
```

## Actions

- start: log "Making the plan for {{ plan_filename }}"
- plan: execute prompt "plan" with plan_filename="{{ plan_filename }}"
- done: log "Plan ready, look in ./issues"

## Description

This workflow creates a step-by-step plan from a specific specification file.
```

### Key Changes

1. Add `Parameters` section defining `plan_filename`
2. Update start action log message to include filename
3. Modify plan action to pass parameter to prompt
4. Update description to reflect specific file processing
5. Add proper Mermaid diagram for clarity

## Implementation Steps

1. Open `builtin/workflows/plan.md`
2. Replace the YAML front matter and content
3. Add Parameters section after front matter
4. Update Actions to use liquid template syntax with parameter
5. Ensure state diagram is properly formatted
6. Test parameter passing works correctly

## Acceptance Criteria

- [ ] Workflow accepts `plan_filename` parameter
- [ ] Parameter is documented in Parameters section
- [ ] Log messages include the filename using liquid templates
- [ ] Parameter is passed to prompt execution
- [ ] State diagram is properly formatted
- [ ] Description updated to reflect specific file processing

## Testing

- Verify workflow can be executed with parameter
- Confirm parameter is properly passed to prompt
- Check log messages display filename correctly
- Ensure liquid template rendering works

## Dependencies

- Requires CLI structure from PLAN_000001
- Must work with existing workflow execution system

## Notes

- Use liquid template syntax: `{{ plan_filename }}`
- Follow existing workflow documentation patterns
- Maintain backward compatibility with workflow engine
- The parameter will be passed from the CLI handler

## Proposed Solution

I have successfully implemented workflow parameter support by modifying the `builtin/workflows/plan.md` file with the following changes:

### Implementation Details

1. **Added Parameters Section**: Added a dedicated Parameters section after the YAML front matter that documents the `plan_filename` parameter:
   ```markdown
   ## Parameters
   
   - plan_filename: The path to the specific plan file to process
   ```

2. **Updated Actions with Liquid Templates**: Modified all relevant actions to use liquid template syntax:
   - **start action**: Changed from `log "Making a plan"` to `log "Making the plan for {{ plan_filename }}"`
   - **plan action**: Changed from `execute prompt "plan"` to `execute prompt "plan" with plan_filename="{{ plan_filename }}"`
   - **done action**: Kept as `log "Plan ready, look in ./issues"` (no parameter needed)

3. **Enhanced Description**: Updated the workflow description from "This workflow works on tests until they all pass" (which was incorrect) to "This workflow creates a step-by-step plan from a specific specification file."

4. **Maintained State Diagram**: Preserved the existing Mermaid state diagram which was already correct.

### Key Features Implemented

✅ **Parameter Definition**: The workflow now clearly documents the `plan_filename` parameter
✅ **Liquid Template Integration**: Uses `{{ plan_filename }}` syntax for dynamic parameter substitution  
✅ **Enhanced Logging**: Log messages now include the specific filename being processed
✅ **Prompt Parameter Passing**: The plan prompt execution now receives the filename parameter
✅ **Backward Compatibility**: Maintains the existing workflow structure and state machine
✅ **Clear Documentation**: Updated description accurately reflects the workflow's purpose

### File Changes Made

The complete updated `builtin/workflows/plan.md` now contains:
- Parameters section defining `plan_filename`
- Updated start action with dynamic filename logging
- Updated plan action with parameter passing to prompt execution
- Corrected workflow description
- Maintained existing state diagram and done action

This implementation satisfies all requirements in the issue specification and enables the CLI command to pass the `plan_filename` parameter to the workflow execution system.