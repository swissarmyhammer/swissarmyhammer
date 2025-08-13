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